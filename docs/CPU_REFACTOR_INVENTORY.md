# CPU refactor source and evidence inventory

Status: baseline inventory captured; the 98 CPU and 182 SIMD files are preserved below pool_cpu/legacy; maintained implementation and source review remain in progress. The separate SIMD executor and adapter registry were removed from the active module graph on 2026-10-31; `Backend::Simd` is now a CPU-plus-Architecture compatibility request. RGB8 Wasm SIMD Convert strategies are restored from archived portable-vector routes, Default RGB→LA/I;16 staging preserves the archived L scratch allocation, and reviewed Convert empty routes retain their archived receipts. The RGBA/RGBa-to-LA vector kernel now lives in a shared native four-byte helper used by both exact mode strategies. The RGBX-to-La/RGBa routes and their inventory were committed in `baf192167`; `0b34e9a9b` exposes same-mode Convert identity in all 25 source contracts, including palette-preserving P/PA and CoreOnly internal high-depth identities. `e75e87f43` routes named ImageOps/Paste color resolution through exact `Mode` and marks internal high-depth color results `NO_PILLOW_SUPPORT`. `7790ed967` classifies reviewed mode-preserving pipeline operations, records RemapPalette and Color3DLut output identities from typed operation contracts, and routes Tiny-RGB eager ExtractBand through prepared CPU architecture execution for CPU Default and compatibility SIMD. `054a98326` replaces clone-only same-carrier PutData prefix writes with the native writer in 16 mode owners, removes copy-only carrier conversion from four PutAlpha and 13 PutAlphaData Default paths, and makes callback-preserving PutData writes validate and mutate the exact materialized DynamicImage byte carrier. `aa6de79ae` carries exact `raster::Mode` through PutData value-kind selection, normalization, immediate-write width selection, and the deferred `PipelineOp::PutData` payload. Current source adds a separate `PutDataValueAt` support contract and passes a cached CPU Default strategy context through callback-driven writes; this source-only follow-up is unverified. GPU consumers match exact modes and explicitly map supported payload identities to the existing shader layout codes; `PixelMode` remains on PutAlpha/PutAlphaData. Cross-mode lazy P/PA output still lacks terminal palette metadata. All current CPU source remains unverified.
Baseline commit: 3f9dfd4036caa061eabcf2255c9f34a745df9c7e
Working checkout: `/Users/lazytrot/work/pillow-rs`, branch `develop` at
`3f110aa47`. The commit includes the exact-mode PutData callback writer and its
sequence-scoped CPU Default context. The current uncommitted source adds a
separate `PutPixelValueAt` support contract for numeric I/F and I;16 writes; the public
wrapper selects it from the raster mode and records the synchronous CPU
strategy receipt. The raw-byte I;16 PutData fast path remains an explicit
TODO. GPU maps exact payload modes to established
shader byte-layout codes; alpha operations keep their separate `PixelMode`
fields. No build, format, lint, test, parity, benchmark, or coverage validation
has run.
Convert's P/PA source matrix is being reconciled against Pillow's `frompalette`
and `topalette` paths; this source remains unverified.
The baseline archive was rechecked on
2026-10-31: all 98 CPU files and 182 SIMD files from
`3f9dfd4036caa061eabcf2255c9f34a745df9c7e` match their preserved contents under
`pool_cpu/legacy/` and `pool_cpu/legacy/pool_simd/`; maintained Rust source has
zero active legacy imports, and the maintained `pool_cpu` tree has no Rayon
scheduling references. This continuation has not run a build, formatter,
checkstyle, lint, test, parity, benchmark, or coverage command.
Related Kata issues: pillow-rs#3ptb (active CPU goal), pillow-rs#brg0 (related parity/performance findings; remains open).

## Current source-only continuation

- Callback-preserving `Image::putdata_value_at_with_context` selects the declared `PutDataValueAt` support contract for the exact mode and writes through the native byte or I;16 carrier. The Pillow and JavaScript sequence adapters retain one CPU Default context for their callback-driven writes; the context caches the selected mode strategy and refreshes it if a re-entrant callback changes the image mode. The synchronous mutation remains visible before the next value is coerced, and telemetry records the selected Default implementation alongside any requested-backend handoff. The direct `Image::putdata_value_at` API creates the same context for one write, while callback-free I;16 value loops reuse one context. The helper retains the existing mode mismatch and out-of-bounds errors. This source-only review has not been validated; direct I;16 byte mutation and wrapper metadata routing remain under audit.
- Numeric I/F `Image::putpixel_mode_scalar` now selects the distinct `PutPixelValueAt` contract from the exact raster mode instead of reporting itself as PutData. It retains the existing numeric-to-four-byte normalization and in-place write through the mode-owned I/F Default implementations, and records the selected CPU Default strategy.
- Direct I;16 PutPixel now uses that same contract with four exact mode classifications and separate `i16`, `i16l`, `i16b`, and `i16n` Default owners. Each owner writes the typed u16 sample in place, preserving modulo-65536 normalization, byte-order identity, and the existing offset error. The broader wrapper/source review and all validation remain pending. These PutPixel updates are unverified.
- PutAlpha keeps One/P source luma borrowed until the required two-channel output is made, updates PA indices and alpha directly in its native two-channel buffer, and replaces La alpha in its native two-channel buffer. PutAlphaData now reads One/P/PA/La directly, uses native three-channel interleaving for RGB-family inputs, and passes existing four-channel source buffers to the alpha-mask writer without cloning. Removed conversions only copied source channels before rebuilding the same declared output; sample values and output identities are unchanged. Semantic color-space promotions remain in their exact mode owners. This source review has not been validated.
- PutData's 16 clone-only same-carrier routes now use `put/helpers/put_data.rs::native`. For complete payloads, the helper constructs the output directly from the supplied bytes; partial and empty payloads retain the source buffer and apply the same byte prefix. This is equivalent to the prior `image.clone()` plus `Prefix::apply` path because those closures performed no sample or mode conversion. Actual `ConvertBuffer` branches remain for cross-layout behavior. This source review has not been validated.
- PutData input-shape selection, byte normalization, and callback-path storage width match exact `raster::Mode` identities instead of first collapsing through `PixelMode`. The byte layouts remain mode-specific: One/L/P are one byte, LA/PA two, RGB/YCbCr/HSV three, and RGBA/RGBa/RGBX/CMYK/I/F use their existing four-byte carriers. The deferred `PipelineOp::PutData` now carries that exact identity as `payload_mode`; CPU contracts and GPU admission, packing, and shader-code selection consume it directly. Unsupported La/LAB/internal high-depth payload modes retain their prior explicit rejection. This source review has not been validated.
- The Convert byte-to-I;16 and native-I;16 vector slice is source-reviewed.
  `One` expands each nonzero byte to 255, `L` widens each byte unchanged, and
  all four I;16 output identities preserve those exact u16 samples. Native
  I;16 inputs retain the archived eight-sample paths for L/LA/RGB/RGBA/CMYK/I/F;
  current Default and vector paths now preserve full u16 values for I and F
  across `I;16`, `I;16L`, `I;16B`, and `I;16N`. The source review found that
  the maintained `I;16N` owner had incorrectly clipped I/F values to 255 even
  though both archived CPU and SIMD paths preserved the full sample. The
  redundant clip strategy is removed and the `I;16N` architecture wrappers now
  use the same native vector helper. Empty One/L→I;16 remains a zero-block
  vector result; empty I;16→byte/scalar targets now select their native route
  and record the archived scalar-control receipt. No validation has run.
- The archived SIMD Convert alpha slice has now been compared against mode
  contracts, architecture guards, dispatch, and kernel bodies. `LA→La` and
  `LA→RGBX` map to `luma_a8`; `La→LA` maps to `la_premultiplied8`; `RGBA→RGBa`
  and `RGBA→RGBX` map to `rgba8`; `RGBa→RGBA/RGB/La/RGBX` map to
  `rgb_premultiplied8`. Premultiply uses the same `(sample * alpha + 127) / 255`
  rounding; unpremultiply retains the same exact reciprocal correction, clamps,
  zero-alpha behavior, and alpha bytes. `RGBa→La` still emits opaque alpha, and
  `LA→RGBX` preserves the source alpha in its fourth byte. Output variants are
  exact. The archived alpha adapter errors when an alpha-transform route emits
  zero vector blocks, except `LA→RGBX`, which returns an empty output. The
  folded CPU contract uses each mode's Default strategy for zero-work alpha
  transforms through nonempty guards; `LA→La` now follows that same fallback.
  `LA→RGBX` keeps its explicit empty result because the archived path supports
  it. Its fourth byte preserves source alpha, and the archived and maintained
  special paths both report the vector route with zero blocks for empty input.
  RGBA/RGBa→LA takes the generic native layout route: both mode-owned wrappers
  now call the exact archived 16-pixel input kernel in
  `convert/helpers/four_byte_to_luma_alpha.rs`, preserving source alpha and
  RGBa's stored premultiplied RGB samples. The helper's eight-pixel output
  block count matches the archived receipt; mode-specific validation errors
  and exact `ImageLumaA8` construction stay in their respective wrappers. This
  maps adapter-level SIMD unsupported errors to the required same-mode CPU
  fallback for zero-work alpha transforms; output and error parity still need
  the planned CPU parity run. This source-only comparison has not been
  validated.
- Source-only review of archived CMYK→RGB is complete. The former tagged
  `ImageRgba8` plus `CMYK` string guard is now the exact `ImageCmyk8` source and
  `ImageRgb8` target contract; matrix requests remain on the separate
  Default-only `ConvertMatrix` route. The maintained SSE2 path retains padded
  16-pixel wide blocks, SSSE3 retains its full-block channel gather with wide
  handling for the final partial block, and NEON retains 16-pixel vector
  groups plus the archived scalar tail. All preserve
  `ink = 255 - K` and the `+128` `muldiv255` rounding before RGB packing. Empty
  inputs produce the archived scalar-control result, and the output variant is
  exactly `ImageRgb8`. No source difference was found in this slice; it remains
  uncompiled and unvalidated.
- Source-only review of archived L→RGB/RGBA expansion is complete. The exact
  16-pixel shuffle tables, partial-block padding, three-byte RGB packing, and
  RGBA alpha fill of 255 are retained in the `luma8` helper reached through the
  SSE2 and NEON mode owners. The old SIMD adapter returned a scalar-control
  empty image before invoking these kernels; the exact-target guards now select
  the architecture owner, and its zero-block path records scalar-control.
  Output shape and bytes remain the same. This route has not been validated.
- The L→LA layout route is also reconciled: both the archived generic block
  helper and maintained `luma8` kernel process eight L samples per output
  vector and emit `[L, 255]` per pixel using the opaque sentinel lane. Empty
  input uses the archived scalar-control output and the maintained zero-block
  helper now records scalar-control under the exact-target guard, with the same
  exact LA result. This route has not been validated.
- The generic zero-pixel Convert interception is reconciled for the reviewed
  RGB8, L, LA, RGBA, RGBX, RGBa generic color/typed targets, and native I;16
  source routes. The archived adapter
  ran alpha-specific conversions first, then recorded scalar-control for an
  accepted empty native-layout conversion. Exact-target guards now admit the
  reviewed architecture owners; their zero-pixel helpers record scalar-control
  without vector-block or scalar-tail counts. RGBA→RGBa/RGBX and other
  alpha transforms that require vector groups retain nonempty guards and
  select Default for zero-work input; `LA→RGBX` keeps its explicit empty route
  because the archived alpha adapter returns that output. Source is unvalidated.
- RGB→RGBA sample conversion is source-reviewed. The archived layout kernel
  and maintained `rgb8` architecture owners both emit each native RGB triplet
  followed by opaque alpha and return exact `ImageRgba8` output. The portable
  four-pixel shuffle and NEON sixteen-pixel structure load/store retain their
  vector widths and scalar tails; nonempty images smaller than a full block
  are entirely scalar tails. Empty images were intercepted by the archived
  scalar-control route; the maintained exact-target guard now selects the
  architecture owner, whose zero-pixel branch records the same scalar-control
  receipt. The shared portable helper and NEON route now use a
  fallible exact reservation and map invalid extents, storage shape, or reserve
  failure to the archived `SIMD Convert is unsupported for the validated
  image layout/parameters` `NotImplementedError`. The archived batch executor
  propagated adapter errors and did not retry on CPU; the prepared architecture
  owner likewise returns its leaf error directly. No validation has run.
- RGB→I;16/I;16L/I;16B/I;16N is Pillow-supported, not a core-only extension.
  The baseline `try_convert_to_luma16` accepts all four target spellings and
  routes RGB through the `L` conversion before widening each byte to native
  `u16`. The maintained RGB8 Default owner computes the same rounded BT.601
  fixed-point projection (`19595/38470/7471`, bias `32768`) into a temporary L
  byte buffer, then widens those samples into the exact requested I;16
  `DynamicImage` variant. This preserves the archived two-buffer allocation
  boundary without constructing or retagging a `DynamicImage` intermediate.
  No RGB8 architecture strategy is declared for these targets, so they remain
  Default-only. The typed L scratch route and output are source-reviewed; no
  validation has run.
- RGB→I/F is source-reviewed against both archived executors. The archived CPU
  and maintained Default owners use the same fixed-point rounded luma formula
  for I, then write the signed value as little-endian i32 into the four-byte
  native carrier. Both use the same integer-weighted `(299R + 587G + 114B) /
  1000.0f32` formula for F and emit little-endian f32 bytes. The archived wide
  Convert kernel and maintained `native_rgb` kernel gather eight RGB pixels,
  run the same lane equations, zero-fill inactive tail lanes, and emit only
  active results. Both return the exact I/F identity. The archived adapter
  accepted exact native RGB bytes and handled empty images as `scalar-control`;
  the maintained support declaration admits the exact I/F targets for the
  RGB8 owner, and its zero-block path records the same empty control receipt.
  SSE2/NEON/Wasm selections use these eight-lane kernels, with the Default strategy
  retained for fallback. No arithmetic or tail difference was found in this
  slice; parity and benchmark validation remain pending.
- RGB→LA is source-reviewed across the archived CPU, archived SIMD, and
  maintained mode owners. The archived CPU path used Pillow's rounded L
  grayscale conversion and filled the LA alpha byte with 255. Its SIMD path
  used eight-pixel fixed-point luma blocks, zero-filled inactive lanes, and
  interleaved opaque alpha. The maintained Default owner computes the same
  rounded luma into an L byte scratch buffer and then emits `[L, 255]`,
  preserving the archived L-buffer allocation followed by the LA output
  allocation without constructing a `DynamicImage` intermediate. The selected
  architecture owner calls the eight-lane `native_rgb::to_luma_alpha_opaque`
  kernel with the same active-lane output. Both return exact `ImageLumaA8`.
  Empty images remain accepted: the archived path records scalar-control and
  the maintained zero-block vector helper records scalar-control, although the
  new support guard can retain the requested architecture selection.
  Pillow's Convert table also explicitly provides RGB→La; the maintained route
  uses the same rounded luma and opaque alpha with the exact
  `ImageLaPremultiplied8` identity. Both alpha targets are Pillow routes, not
  core-only extensions. No validation has run.
- RGB→L is source-reviewed against the actual archived Convert route. Its
  `try_layout_special_conversion` delegated exact RGB8 input to the archived
  Grayscale kernel selector, so the generic eight-lane Convert luma helper was
  not the executed path. The maintained RGB8 Convert owner delegates to the
  same Grayscale architecture kernels: AVX2 and NEON retain their specialized
  16-pixel loops, while SSE2/SSSE3/Wasm use the archived 16-pixel interleaved
  `wide` loop; all use the same rounded Pillow luma result and padded active
  tail. The Default route reuses the same serial grayscale kernel as the
  archived CPU route. Both SIMD routes preserve exact `ImageLuma8`; zero-block
  execution records scalar-control in the archived adapter and maintained
  `vector_output`. The typed declaration selects each concrete architecture
  from the injected context and retains Default fallback. No source difference
  was found for this route; parity and performance validation remain pending.
- RGB→CMYK/YCbCr/HSV is source-reviewed across the archived CPU, archived
  Convert SIMD, and maintained mode owners. CMYK keeps the direct inverse
  channels `[255-R, 255-G, 255-B, 0]`; its archived and maintained vector
  kernels process four pixels per block. YCbCr keeps the same eight lookup
  tables, fixed-point shift, neutral offset, and clamp in eight-pixel blocks.
  HSV keeps the same eight-pixel wide-float route, red-then-green-then-blue
  maximum tie order, and intermediate f32 hue narrowing before wrap and byte
  truncation. The maintained Default owners call the existing native RGB
  conversions; SSE2/NEON owners call the corresponding native vector helpers.
  All preserve padded inactive lanes, active-only stores, scalar-control for
  empty input, and exact `ImageCmyk8`, `ImageYCbCr8`, or `ImageHsv8` identity.
  No source arithmetic or tail difference was found. The archived portable
  SIMD executor dispatched these `wide` kernels without target-architecture
  gates; `wide` 1.6.1 maps the required `u8x16`, `i32x8`, `f32x8`, and `f64x8`
  operations onto Wasm SIMD128 when `simd128` is enabled. The maintained RGB8
  owner had omitted those Wasm strategies. It now declares and routes them for
  these targets, along with LA, Pillow-defined La, I/F, RGB identity, and the
  four-byte expansion targets. The shared portable RGB→RGBA kernel now lives
  under the RGB8 mode helpers and is used by SSE2 and Wasm; NEON retains its
  sixteen-pixel structure kernel. Other family/architecture rows still need
  the same build-availability audit. No build or validation has run.
- RGB→RGB identity is source-reviewed. The archived generic layout route and
  maintained SSE2/NEON/Wasm mode owners both copy native triplets through five-pixel
  `u8x16` blocks, including a padded final block, and preserve the exact
  `ImageRgb8` identity. The archive returned an empty image through its
  scalar-control path; the maintained exact-target architecture guard now
  selects the identity kernel, whose zero-block receipt is also scalar-control.
  Nonempty output and vector-block accounting agree. Other empty-route receipts
  remain part of the broader fallback reconciliation; no validation has run.
- RGB→La, RGBa, and RGBX are Pillow-supported pairs in Pillow's
  [Convert table](https://github.com/python-pillow/Pillow/blob/main/src/libImaging/Convert.c#L1412-L1428).
  Default, SSE2, NEON, and Wasm produce the exact requested `DynamicImage`
  variant from native RGB samples and insert an opaque fourth byte; that byte
  is alpha for RGBa and padding for RGBX. The architecture owners reuse the
  reviewed RGBA expansion kernel. Pillow's `topalette` route accepts RGB for
  either P or PA and supplies opaque alpha for PA. Its maintained Default
  owner uses native RGB samples for those palette outputs; parity remains
  pending.

- RGB8→LAB leaf execution is source-reviewed as a scalar-control migration.
  The archived SIMD adapter accepted only exact RGB8 input and Pillow's RGB
  identity, recorded `scalar-control`, then called the shared LAB converter;
  it contained no vector kernel. The maintained `ConvertLab` support contract
  accepts exact RGB8 native bytes and declares Default only. Its `rgb8` Default
  owner retains the archived 256-entry axis mapping, LittleCMS tetrahedral
  tie order, 0x8001 interpolation bias, and byte packing, while returning the
  now-distinct `ImageLab8` variant. The enclosing typed Convert plan first
  selects and executes source normalization, then selects this exact RGB8
  child route. Empty RGB8 images remain accepted with an empty LAB result.
  This source comparison has not been validated.
- RGB8→P/PA remains a Default-only Pillow palette route. The archived SIMD Convert
  target descriptor has no palette or palette-alpha target, and the maintained
  RGB8 architecture declarations likewise contain no P/PA specialization.
  The mode owner invokes the existing web-palette quantizer for P and the
  palette-alpha helper for PA with opaque source alpha, preserving exact
  `ImagePalette8`/`ImagePaletteAlpha8` output. P installs the WEB palette and
  keeps `PreserveSource` wrapper metadata; PA uses the alpha-capable palette
  helper with `ExifOnly` metadata. Pillow's native `topalette` converter
  accepts RGB sources for both P and PA; its Python RGBA→PA special case only
  changes how source alpha is preserved. Both RGB8 palette targets remain
  Pillow-scoped. This is an explicit Default destination, not a missing
  architecture kernel; parity validation remains pending.

- Convert-to-LAB is now declared as a composed Convert source/target route at
  the operation boundary. Pillow identities report Pillow scope; internal
  high-depth sources report CoreOnly; La retains the exact L-base rejection.
  Prepared execution captures source normalization and RGB8 ConvertLab
  selections before running, and the wrapper admits its lazy graph through the
  same source/target declaration. The I;16 route retains the existing clipped-L
  normalization. This change is committed as `5e4b509d4`; it remains
  uncompiled and unvalidated, and the archived SIMD Convert reconciliation is
  still pending.
- Prepared palette conversions now cover `One`/`L`/`LA`, scalar `I`/`F`, and
  all four exact I;16 identities to `P`/`PA`, plus all five internal
  high-depth identities to P/PA, RGB→P/PA, RGBA→P/PA,
  RGBa→P/PA, RGBX→P/PA,
  LAB→P/PA, CMYK/YCbCr/HSV→P/PA, and `P`↔`PA`. They select exact Default Convert
  strategies through the injected CPU context and return the index raster,
  palette data, and wrapper metadata policy separately. Native kernels preserve
  bilevel expansion, byte-luma indexing, I/F reduction and clamp behavior, RGB
  WEB-palette quantization with opaque per-pixel alpha in PA, RGBA FASTOCTREE
  quantization with its per-entry alpha palette for P, RGBA median-cut RGB
  quantization with preserved source alpha for PA, native CMYK/YCbCr/HSV color
  transforms before WEB quantization, native u16 clipping directly into
  identity-palette indices for all I;16 variants, raw LAB/RGBa sample
  quantization, RGBX padding-byte removal, and indexed alpha semantics. The
  five internal high-depth source modes now declare core-only P/PA targets and
  map native samples directly into WEB quantization RGB triples
  without constructing an intermediate RGB raster; P preserves source
  metadata and PA retains EXIF only. These `NO_PILLOW_SUPPORT` routes need
  target-only parity coverage across CPU Architecture requests. Every other
  accepted P/PA source pair uses its prepared strategy; `La` to other targets
  remains an explicit validation error. The generic P/PA wrapper quantizers
  were removed after reviewing that reachability. Lazy
  Cross-mode `PipelineOp::Convert` palette targets remain an explicit TODO until
  the lazy pipeline terminal can carry generated palette metadata; same-mode P/PA
  identity is admitted and preserves the attached palette. No unit tests or validation
  commands were run.
- Module-level `Image.merge` now accepts the canonical `raster::Mode` in Rust;
  Python and JavaScript parse their external Pillow strings before calling it.
  Merge validation reads each band's typed mode identity instead of comparing
  mode strings. Blend compatibility and Composite mask/palette checks also use
  typed identities. Composite's cross-mode call now passes the exact typed
  target to Convert. `PipelineOp::Merge` carries the exact typed target `Mode`,
  preserving aliases and each I;16 identity without a parallel mode string.
  The public Convert boundary is typed; its prepared CPU source/target matrix,
  archived-kernel reconciliation, and caller review remain pending under
  `pillow-rs#3ptb`. Prepared CPU strategies for Merge, BlendModule, and
  CompositeModule have since been added as selective local changes; their
  source review and validation remain pending.
- Palette-producing materialized paths now tag their buffers directly as
  `DynamicImage::ImagePalette8` or `DynamicImage::ImagePaletteAlpha8`;
  `LoadedData` no longer stores a parallel `explicit_mode` or codec
  `decoded_mode`. `Image::putpalette` selects its source branch from typed
  `Mode`, and a `PalettedData` image reads its identity from the exact mode
  tag on its index raster. The lazy P result retains its planned output mode because it must
  restore the palette wrapper when loaded.
- Binary conversion and font-mask construction now create the exact `One`
  variant directly. Quantize's lazy P source likewise uses `ImagePalette8`;
  the pipeline retains its planned P output identity for palette restoration.
- Native two-channel readers used by GetData, GetColors, GetExtrema, Entropy,
  Histogram, GetBBox, and GetProjection now match palette-alpha and
  premultiplied-luma-alpha variants where they share the same raw channel
  semantics. These are focused carrier-preservation edits; they do not finish
  those operations' exhaustive mode contracts.
- `Image::getchannel`, `Image::putalpha`, and `Image::putdata` now use exact
  `raster::Mode` for mode decisions. The I;16 `putdata` byte order is selected
  from the exact I;16 variant. `putalpha_data` validates its materialized mask
  by its exact variant. The older `PixelMode` remains only as the normalized
  byte-payload descriptor at pipeline/byte-layout boundaries. These
  source-only edits preserve existing string diagnostics and are unverified.
- The exact shared raster variants expose remaining GPU compatibility work.
  `compute/pool_gpu/native_input.rs` now carries a TODO to audit input packing
  and admission for exact palette and premultiplied variants. CPU work is still
  incomplete; no build, formatting, checkstyle, lint, test, parity, benchmark,
  or coverage command has run.
- Eager generated results now pass through `DynamicImage::with_mode` and keep
  that exact variant before wrapper construction. P gradients accept only a
  compatible one-byte sample carrier; the prior fallback that converted an
  arbitrary raster to Luma before treating it as palette indices is removed.
  Incompatible generated storage returns an error. `Image::from_dynamic`
  wraps an already mode-tagged raster; explicit carrier tagging is isolated in
  the fallible `Image::from_dynamic_with_mode` adapter. Loaded mode access and
  GPU cached-mode fallback now read the `DynamicImage` variant instead of
  inferring mode from pixel color type. Pipeline output identity remains typed
  metadata until the pending graph materializes. `Image::mode_from_materialized`
  now projects the concrete `DynamicImage` tag directly, and successful cached
  materializations take precedence over header or planned identity in
  `known_mode_identity()`. A pending `explicit_mode` therefore cannot override
  a materialized result. This source change remains unverified.
- Convert's shared native RGB transform kernel now receives the exact source
  `Mode` from each RGB/RGBA/RGBX/RGBa strategy, and LA's I/F conversion records
  typed output identities. Pad's resize-window helper also receives `Mode`
  instead of a Pillow spelling. These changes keep mode selection at the
  strategy boundary while preserving existing diagnostic text. Source-only;
  unverified.
- AlphaComposite now has a maintained `composite` owner with exhaustive
  25-mode support classification, exact LA/RGBA strategies, same-mode and
  equal-size secondary-image admission, and native serial Default arithmetic.
  Its prepared plan is registered through the injected CPU context and does
  not convert between modes or schedule parallel work. The archived eight-pixel
  vector kernel now lives in a shared maintained helper and is selected from
  the declared SSE2, NEON, or wasm SIMD strategy only when its exact byte layout
  and minimum vector size are eligible; its archived exhaustive arithmetic
  cases are colocated but remain unrun. CPU uses no parallel scheduler. The
  fused AlphaComposite+Mirror route remains pending. The archived serial path
  ignores nonzero `dest`/`src`, so preserve that behavior until the frozen CPU
  cases establish the offset contract. Source-only; unverified.
- GetProjection's direct eager route and RGB/CMYK PutPixel terminal now use the
  maintained operation-owned projection strategies. The terminal retains its
  exact `Mode`, uses CPU architecture implementations for compatibility SIMD
  requests, preserves the CMYK positive-write fast path, and takes its
  `ExecutionContext` from the prepared backend plan instead of creating a
  second context. Searches of maintained source find no calls to the archived
  SIMD RGB/CMYK projection adapters or old SIMD terminal executor. GPU terminal
  support and admission still have an explicit TODO. Source-only; unverified.
- Direct LinearGradient now has one operation-level contract classifying all
  25 identities, with exact One, L, P, I, and F strategies and the existing
  ValueError for other modes. Each accepted mode constructs its own exact
  `DynamicImage` variant through a Default implementation; the archived
  `wide::u8x16` row fill is selected through the injected CPU architecture
  context for compiled SSE2, NEON, and Wasm SIMD128 implementations. The public
  entry no longer converts `Mode` back to a Pillow string or branches between
  the CPU Default and compatibility SIMD request paths. RadialGradient now has the same
  exhaustive mode contract for One, L, P, I, and F, with native mode-specific
  outputs; its shared distance formula and fixed L template are separate from
  the L architecture copy strategy. Both public entries use the injected
  architecture context; the compatibility request uses the same CPU owner.
  EffectMandelbrot now declares its fixed L output, typed size/extent/quality
  contract, exact L Default implementation, and width-guarded AVX, SSE2, NEON,
  and Wasm SIMD128 paths. Its previous eight-lane kernel retains the same
  recurrence order and reports vector work through the prepared receipt. The
  zero-size shortcut and argument error order remain at the public entry.
  Source-only; unverified.
- Full GetData byte export now matches the exact `Mode` with the same
  `DynamicImage` variant for all 25 identities instead of selecting a conversion
  from `ColorType`. LAB's signed-byte offset, raw I/F carrier bytes, and I;16
  clipping are explicit; internal high-depth samples narrow only at this byte
  return boundary. The allocated band path now validates against exact mode
  arity and selects native carriers, including LAB, I/F, I;16, and internal
  high-depth samples, without widening the whole raster to RGBA. Typed band
  formatted output and scalar extraction now use `Mode` as well. The allocated
  band API still needs the prepared backend/architecture route, and the
  caller-owned path still needs a direct support declaration that derives its
  CPU/GPU selection. Source-only; unverified.
- ImageOps Posterize and Solarize now have CoreOnly P strategies that transform
  native palette-index bytes and preserve the wrapper's palette metadata.
  Default uses scalar mode-owned kernels; SSE2, NEON, and Wasm SIMD128 use the
  existing native-byte point kernel under P-specific support declarations.
  Palette-aware lazy execution now keeps P indexes and their palette attached
  for these operations and for single-band Eval LUTs, including Solarize's
  custom/high-threshold path. `NO_PILLOW_SUPPORT` is declared beside the P
  strategies because Pillow's ImageOps `_lut` rejects P. No tests or validation
  commands were run; operation × exact Mode × Architecture parity remains
  pending.

### Paste source audit (maintained strategies written; source review continues)

- Public admission lives in `pillow-rs/src/ops/paste/operations.rs`. It reads
  the destination's typed `Mode`, validates placement and source/mask region
  dimensions, derives `mask_alpha` from the exact mask mode, then queues
  `PipelineOp::Paste`; output preserves destination identity and metadata.
  `1`/`L` masks use their luma sample; `LA`/`RGBA`/`RGBa` masks use the alpha
  sample, with `RGBa` retaining its premultiplied PREBLEND rule. Other masks
  return `ValueError("bad transparency mask")`. `resolve_image_source`
  preserves same-mode inputs, preserves `LA`/`RGBA`/`RGBa` carriers for an RGB
  destination, preserves an unmasked RGB source for RGBA output, and promotes
  P indices to opaque PA. Other conversions are explicit at this wrapper
  boundary. Solid sources are built in the exact destination mode, including
  raw I/F bytes and typed I;16 values.
- Named Paste colors now pass their exact destination `Mode` to
  `color::getcolor_for_mode` after parsing color text; the core no longer
  converts the mode to a Pillow spelling for this route. Internal high-depth
  results are marked `NO_PILLOW_SUPPORT` in the shared typed resolver.
- The baseline CPU implementation is `pool_cpu/legacy/ops/paste.rs`, with row
  kernels in `paste/kernels.rs` and typed/mask layout helpers in
  `paste/layout.rs`. `paste_native_rows` copies the clipped intersection of
  exact same-mode byte samples; its byte layouts are 1/L/P (1 byte),
  LA/La/PA (2), RGB/HSV/YCbCr/LAB (3), and RGBA/RGBa/RGBX/CMYK/I/F (4).
  P/PA sources materialize their index plane. The unmasked RGB-to-RGBA kernel
  copies RGB and writes alpha 255. Empty overlap returns the original
  destination. The archived CPU `paste_native_masked` fast path owns
  1/L/P/LA/PA/RGB/HSV/RGBA/CMYK; its non-binary 1/P requests and the remaining
  byte modes flow through `paste_rgba_fallback`. The maintained implementation
  must preserve those raw-sample results with exact-mode loops, not widen a
  raster to RGBA for kernel reuse.
- For same-mode byte storage, L masks apply
  `(source*mask + destination*(255-mask) + 127)/255` independently to every
  stored byte, including RGBA alpha, CMYK K, and both LA/PA samples. `1`
  masks copy the entire sample when nonzero. LA/RGBA masks select their alpha
  byte; RGBa masks select premultiplied alpha and use Pillow PREBLEND
  `((((destination*(255-mask)+128)>>8) + destination*(255-mask)+128)>>8) +
  source`, including at a zero mask. For the RGB destination's retained
  LA/RGBA/RGBa source routes, the archived fallback expands LA luma into the
  three RGB channels or drops the source's fourth byte, then blends only the
  destination RGB bytes; source alpha is not copied into the RGB result.
- All four I;16 identities use the typed u16 carrier. Unmasked and binary-mask
  routes copy complete samples. A fractional byte mask advances the image8
  pointer by one byte per pixel and blends only that byte, preserving the
  other byte. I;16L, I;16B, native I;16, and I;16N each select byte order
  explicitly; RGBa masks keep PREBLEND at zero. Do not normalize this to a
  u16 blend. I/F preserve their raw four-byte carrier and blend those bytes.
  No archived exact-mode Paste strategy exists for the five internal
  high-depth identities. The maintained implementation adds native typed
  strategies for them and marks each core-only route with a nearby
  `NO_PILLOW_SUPPORT` comment.
- The archived SIMD adapter is `legacy/pool_simd/ops/adapters/paste/`. Its
  native-copy planner covers the 14 byte identities `1`, L, P, LA, PA, RGB,
  HSV, YCbCr, RGBA, CMYK, RGBa, RGBX, I, and F, plus all four I;16 identities.
  (La and LAB have CPU Default behavior but no SIMD Paste plan.) Its planner
  admits binary-mask copy for every listed layout and fractional masks for
  L/LA/PA/RGB/HSV/RGBA/CMYK/I;16. It includes a `Broadcast` source layout for
  the 1x1 masked bitmap source and direct unmasked RGBA-to-RGB and RGB-to-RGBA
  routes. The 8/16-lane `wide` copy/blend kernels keep scalar tails; explicit
  AArch64 NEON blocks exist for L/LA/RGBA/RGB/CMYK L-mask blends. I;16 uses
  eight u16 lanes and preserves the selected stored byte. Its adapter also
  contains `simd_composite_module`, which reuses these native mask/copy kernels;
  the maintained CompositeModule owner may call shared Paste helpers directly.
  Rayon row scheduling in either archive does not belong to CPU and remains a
  later `pool_parallel` concern.
- Maintained placement and caller rewiring: `pool_cpu/ops/paste/` now has
  family orchestration, all 25 exact mode owners, shared input/byte-layout
  helpers, per-mode Default implementations, and architecture-selected SSE2,
  NEON, and Wasm SIMD128 routes for byte identities and the four I;16 modes.
  `OperationId::Paste` has an exhaustive support contract, closed prepared
  plan, and `prepare_pipeline`; the effects registry calls it directly and the
  old `op_paste` import is removed. The shared `PipelineOp::Paste` GPU route
  remains intact. Host-value normalization stays in
  `pillow-rs/src/ops/paste`, preserving its calls from `Image::paste`,
  `Image::paste_at`, `paste_bitmap_solid_masked_at`, `draw/bitmap.rs`, and
  `ops/crop.rs`.
- Archived function destinations: CPU `op_paste` becomes family orchestration;
  `paste_region`, checked row geometry, and compact source expansion become
  the Paste input/region plan; native row copy and RGB-to-RGBA copy become
  exact mode Default implementations; masked L/LA/PA/RGB/HSV/RGBA/CMYK rows
  become their exact mode Default kernels; I;16 fallback becomes four exact
  typed mode implementations sharing only byte-order-aware sample helpers.
  SIMD `plan.rs` becomes linked input/architecture guards; `apply.rs` and
  `apply/generic.rs` become shared vector row helpers; `blend.rs` becomes
  shared DIV255/PREBLEND vector arithmetic; `apply/{l,la,pa,rgb,rgba,cmyk_l}.rs`
  maps to exact mode/architecture implementations; `luma.rs` and
  `rgb_cmyk.rs` contain the explicit NEON kernels; `luma16.rs` maps to the four
  I;16 architecture owners; and `rgb_to_rgba.rs` plus `mod.rs`'s RGBA-to-RGB
  path map to exact cross-mode source strategies. Archived SIMD cases remain
  review inputs for parity selection; the existing CPU and SIMD tests in
  `legacy/` remain inert.
- Archived CPU/SIMD Paste behavior and public callers were inspected and are
  recorded above; the full maintained-route review remains open. Maintained
  support is mode-specific and drives prepared
  strategy selection. Shared `wide` kernels implement the selected vector
  routes; the exact L-mask, LA-with-L-mask, RGB-with-L-mask, RGBA-with-L-mask,
  and CMYK-with-L-mask NEON blends are restored in their exact mode folders.
  The unmasked RGB-to-RGBA and RGBA-to-RGB shuffles now use exact destination
  mode helpers and shared native-byte channel kernels. Five internal
  high-depth identities have typed
  core-only implementations marked `NO_PILLOW_SUPPORT`. All inventoried
  explicit NEON blends and cross-layout shuffle routes now have maintained
  owners. No Paste unit tests are being added; parity must cover every mode and
  available architecture, including mask arithmetic, clipping, typed I;16
  byte-lane behavior, alpha selection, premultiplied PREBLEND, and telemetry.
  The exact L-mask NEON subkernel admission is now in the five mode support
  declarations: same-mode native source, nonbinary L mask, no compact
  broadcast, nonempty clipped overlap, and clipped row-width minima of 16
  pixels for L, 8 for LA, 16 for RGB, 4 for RGBA, and 16 for CMYK. The generic
  architecture vector strategy remains selectable for other Paste inputs. The
  archived Rayon clipping case moves to `pool_parallel`. Top-level caller
  wiring is reconciled; per-mode implementation and support review is still
  open, so this is not full family verification. This source-only
  migration slice is unverified; no build,
  formatter, checkstyle, lint, test, parity, benchmark, or coverage command
  has run for Paste.

This inventory records the source and evidence denominator before rebuilding pool_cpu. It is a migration ledger, not a claim that the listed mode literals prove support. Each function's exact mode support, output identity, rejection behavior, shared helpers, and architecture fallback must be confirmed from its implementation before it is copied into the maintained tree. The source path lists are taken from the baseline Git tree, so every archived file can be checked against 3f9dfd403.

## 1. Baseline and preservation boundary

- pool_cpu: 98 tracked Rust files: 2 pool-level files and 96 files under ops.
- pool_simd: 182 tracked Rust files: 4 pool-level files and 178 files under ops.
- PipelineOp: 84 baseline variants assigned to 21 operation families. Exact variant ownership is frozen in Section 5 of IMPLEMENTATION_STRUCTURE.md.
- No pool_parallel existed at baseline.
- At the baseline, raster::DynamicImage has ten storage variants and no exact mode tag. The public Pillow image wrapper carries logical mode outside that enum. The maintained enum expansion is in progress and has not been compiled or validated.
- The baseline raster storage variants are ImageLuma8, ImageLumaA8, ImageRgb8, ImageRgba8, ImageLuma16, ImageLumaA16, ImageRgb16, ImageRgba16, ImageRgb32F, and ImageRgba32F. This layout cannot distinguish multiple Pillow identities sharing a storage buffer.
- Current mode carriers to reconcile: `LoadedData` derives identity only from its exact `DynamicImage` variant. Pillow-owned codec adapters translate image-slash-star `ImageMode` at encoded boundaries; that codec mode is no longer stored beside loaded pixels. `Image::Pipeline.explicit_mode`, `PipelineModeCache`, and `known_pipeline_op_mode` still store and propagate typed `Mode` for pending outputs. Pipeline mode-preservation metadata classifies Transform, AlphaComposite, BlendModule, and CompositeModule as same-mode operations based on their output contracts; the transition planner also takes RemapPalette's P output and Color3DLut's exact target from their operation semantics. This permits direct mode reporting and safe prefix reuse for those reviewed routes but does not complete explicit-mode/cache reconciliation. `Image::from_dynamic` takes a mode-tagged raster; fallible `Image::from_dynamic_with_mode` remains at reviewed adapter boundaries. `Draw::new` accepts `Option<Mode>`; Python and JavaScript parse optional Pillow strings before calling it. `Image::mode()` remains the Pillow-compatible string projection. `PipelineOp` keeps typed `ColorMode` and `PixelMode` inputs; Merge still retains `logical_mode: String`. Some CPU compatibility leaf functions still receive Pillow strings; the separate SIMD executor has been removed. Keep destination modes and per-operation mode parameters where they express operation inputs, and use the exact mode tag on `DynamicImage` for materialized sources.
- Existing ColorMode is declared in pipeline/support.rs and includes Mode1, P, PA, L, LA, LaPremultiplied, RGB, RGBA, RgbPremultiplied, Rgbx, CMYK, YCbCr, HSV, I, F, and one aggregate I16. Existing PixelMode covers L, LA, RGB, RGBA, P, PA, CMYK, Mode1, YCbCr, HSV, I, and F. Both need an explicit mapping to the exact raster mode identities, including four distinct I;16 variants; neither is the image identity source.
- Mode boundaries to update include raster/dynamic.rs construction/decode adapters and transforms; image.rs LoadedData, PalettedData, Pipeline and caches; image construction/metadata/palette modules; pipeline.rs operation descriptors and transition planning; compute routing and every mode-string consumer in CPU/GPU compatibility paths; and the bindings' existing mode/decode handoff. Preserve image-slash-star ownership of EncodedImage and DecodedImage.
- The target has 25 exact Rust identities: 20 existing Pillow mode identities plus five internal high-depth representations. The authoritative variant and folder mapping is in [Implementation structure, Section 2](IMPLEMENTATION_STRUCTURE.md#2-mode-bearing-raster); this inventory does not define a separate path grammar.
- Design amendment: every operation exposes its own typed support contract and composes mode and architecture strategies. The declaration binds support to executable selection and names guards, output identity, build availability, and fallback/rejection reasons. `OperationSupport::composed_supports()` enumerates linked child contracts; prepared plans expose actual child selections. The top-level descriptor and actual prepared selection follow [Implementation structure, Section 3](IMPLEMENTATION_STRUCTURE.md#3-injected-execution-context). This is required target behavior, not a claim that every in-progress operation implements it.
- Ownership amendment: pillow-rs owns the exact Mode selector and mode-bearing DynamicImage. image-slash-star retains codec types and format metadata, translated by Pillow-owned decode/encode adapters. Python parses Pillow strings before typed core calls. See [the mode ownership contract](IMPLEMENTATION_STRUCTURE.md#21-mode-ownership-and-codec-boundaries); implementation remains in progress and unvalidated.
- Crosswalk source: revision 3f9dfd4036caa061eabcf2255c9f34a745df9c7e; 922 input files, SHA-256 d2268b046270c9a7f5816e9549baf797cdd338f5ecb12126ec5bfbffdd2daf6e. Static analysis found 84 variants and no unresolved module edges, but it does not prove runtime reachability or execution.
- Crosswalk surface summary: 214 declared public operations, 214 mapped to Python bindings, 108 PyO3 core API calls mapped, 94 direct receiver-method routes mapped, 160 Python-native calls mapped to registered PyO3 methods, and 81 registry keys. It reports zero unresolved public binding calls, pipeline variants, module edges, fixture operation references, or benchmark-to-parity references. These are static mappings; dynamic callbacks and execution paths remain review items.
- Worktrees inventoried: main at the baseline, /Users/lazytrot/.codex/worktrees/cpu-benchmark-baseline/pillow-rs at c5104cbaa (detached, dirty and preserved), and /private/tmp/pillow-rs-cpu-baseline-e843 at e843f864 (detached and preserved). The main tree was [ahead 1] of origin/main; 144 refs under refs/codex were preserved. No worktree/ref was pruned.
- Kata issue pillow-rs#3ptb tracks the current implementation and remaining completion gates. The continuation goal is recorded in [CPU refactor plan](CPU_REFACTOR_PLAN.md#goal-prompt); preserve in-progress work when continuing it.

## 2. Archive moves and active ownership

Stage 1 preservation is complete: all 98 pool_cpu paths below moved intact under pool_cpu/legacy/, preserving each listed path relative to pool_cpu. All 182 pool_simd paths moved intact under pool_cpu/legacy/pool_simd/, preserving each path relative to pool_simd. Both archives stay outside the compiled module graph. Exact supported/rejected mode contracts and output transitions will be recorded in this ledger from the preserved source before each family's maintained implementation is written.

Active CPU operation ownership follows the family table in IMPLEMENTATION_STRUCTURE.md. In particular, old pool_cpu single-file families are split into family orchestration, exact mode implementations, and Default or actual architecture implementation files. Existing SIMD architecture kernels are folded into the same family/mode owner. The table below gives the frozen source directory groups and target owner; individual functions are accounted for when the family source is reviewed, before the maintained implementation is finalized.

- Active-source dependency audit: searches of maintained Rust source found no imports or module declarations for `pool_cpu::legacy`, `super::legacy`, or `mod legacy`; searches under maintained `compute/pool_cpu` and `compute/registry` found no parallel feature gates, Rayon calls, or parallel iterator markers. This confirms archive isolation and the current serial CPU boundary only; it does not prove that every operation family or route is implemented, compiled, or verified.

CPU paths already match the intended family names except for the old geometry and imageops split: all PipelineOp ownership is the explicit 21-family mapping in IMPLEMENTATION_STRUCTURE.md. Pool-level batch execution becomes executor/batch orchestration; the old pool and ops mod.rs files are archive-only. The allocation test source is test material and has no maintained runtime destination.

The 43 old SIMD adapter groups are assigned to these CPU families. Subdirectories beneath each adapter stay with that family unless the source review proves that a helper is shared at a narrower valid scope.

| SIMD source group | CPU destination owner |
| --- | --- |
| alpha_composite, merge, module_blend, native_merge | composite |
| box_blur, filter, gaussian_blur, median_filter, neighborhood, native_filter | filter |
| chops, duplicate, native_arithmetic, offset | chops |
| color3dlut | color3dlut |
| convert | color |
| colorize, expand, histogram, lut | imageops |
| crop | crop |
| draw | draw |
| effects, gradient, mandelbrot | effects and direct image-generation entry points |
| enhance | enhance |
| extract_band, native_channel, put_alpha, put_data, put_pixel, put_pixel_batch | put |
| flip, transpose | transpose |
| grayscale | grayscale |
| pad | geometry |
| paste | paste |
| reduce | reduce |
| resize | resize; Pad remains geometry-owned, with shared kernels placed only after source/caller review |
| rotate | rotate |
| transform | transform |
| adapters.rs, layout, native_bytes, pipeline, projection, support, point_batch, batch_execution, pool facade, ops facade | registry/executor or the narrow shared helper owner established from imports and callers; these are not independent operation families |

### CPU source groups
### (ops module) (1 files)
- pillow-rs/src/compute/pool_cpu/ops/mod.rs

### (pool support) (2 files)
- pillow-rs/src/compute/pool_cpu/batch_execution.rs
- pillow-rs/src/compute/pool_cpu/mod.rs

### chops (1 files)
- pillow-rs/src/compute/pool_cpu/ops/chops.rs

### color (1 files)
- pillow-rs/src/compute/pool_cpu/ops/color.rs

### color3dlut (1 files)
- pillow-rs/src/compute/pool_cpu/ops/color3dlut.rs

### color_convert_allocation_tests (1 files)
- pillow-rs/src/compute/pool_cpu/ops/color_convert_allocation_tests.rs

### composite (2 files)
- pillow-rs/src/compute/pool_cpu/ops/composite.rs
- pillow-rs/src/compute/pool_cpu/ops/composite/tests.rs

### crop (1 files)
- pillow-rs/src/compute/pool_cpu/ops/crop.rs

### draw (10 files)
- pillow-rs/src/compute/pool_cpu/ops/draw.rs
- pillow-rs/src/compute/pool_cpu/ops/draw/arcs.rs
- pillow-rs/src/compute/pool_cpu/ops/draw/canvas.rs
- pillow-rs/src/compute/pool_cpu/ops/draw/clipping.rs
- pillow-rs/src/compute/pool_cpu/ops/draw/dispatch.rs
- pillow-rs/src/compute/pool_cpu/ops/draw/ellipse.rs
- pillow-rs/src/compute/pool_cpu/ops/draw/operations.rs
- pillow-rs/src/compute/pool_cpu/ops/draw/polygon.rs
- pillow-rs/src/compute/pool_cpu/ops/draw/primitives.rs
- pillow-rs/src/compute/pool_cpu/ops/draw/rounded_rect.rs

### effects (1 files)
- pillow-rs/src/compute/pool_cpu/ops/effects.rs

### enhance (1 files)
- pillow-rs/src/compute/pool_cpu/ops/enhance.rs

### eval (2 files)
- pillow-rs/src/compute/pool_cpu/ops/eval.rs
- pillow-rs/src/compute/pool_cpu/ops/eval/tests.rs

### filter (27 files)
- pillow-rs/src/compute/pool_cpu/ops/filter.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/blur.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/blur/box_rows.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/blur/line.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/blur/radius_one.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/blur/radius_one/vertical.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/blur/tests.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/convolution.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/filter3x3.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/filter3x3/tests.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/filter5x5.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/filter5x5/int32.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/filter5x5/int32/bounded_binomial.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/filter5x5/int32/tests.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/filter5x5/native_bytes.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/filter5x5/native_bytes/binomial.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/filter5x5/native_bytes/tests.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/kernels/blur_line_steps.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/kernels/i32_cross_smooth.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/kernels/mod.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/rank.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/rank/float.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/rank/tests.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/rgb_blur_ring.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/uniform_box.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/uniform_box_tests.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/uniform_image.rs

### geometry (1 files)
- pillow-rs/src/compute/pool_cpu/ops/geometry.rs

### grayscale (1 files)
- pillow-rs/src/compute/pool_cpu/ops/grayscale.rs

### imageops (9 files)
- pillow-rs/src/compute/pool_cpu/ops/imageops.rs
- pillow-rs/src/compute/pool_cpu/ops/imageops/colorize.rs
- pillow-rs/src/compute/pool_cpu/ops/imageops/expand.rs
- pillow-rs/src/compute/pool_cpu/ops/imageops/geometry.rs
- pillow-rs/src/compute/pool_cpu/ops/imageops/histogram.rs
- pillow-rs/src/compute/pool_cpu/ops/imageops/pad.rs
- pillow-rs/src/compute/pool_cpu/ops/imageops/pad/native.rs
- pillow-rs/src/compute/pool_cpu/ops/imageops/pad/tests.rs
- pillow-rs/src/compute/pool_cpu/ops/imageops/pixel.rs

### paste (4 files)
- pillow-rs/src/compute/pool_cpu/ops/paste.rs
- pillow-rs/src/compute/pool_cpu/ops/paste/kernels.rs
- pillow-rs/src/compute/pool_cpu/ops/paste/layout.rs
- pillow-rs/src/compute/pool_cpu/ops/paste/tests.rs

### put (1 files)
- pillow-rs/src/compute/pool_cpu/ops/put.rs

### reduce (4 files)
- pillow-rs/src/compute/pool_cpu/ops/reduce.rs
- pillow-rs/src/compute/pool_cpu/ops/reduce/kernels.rs
- pillow-rs/src/compute/pool_cpu/ops/reduce/kernels/rgb.rs
- pillow-rs/src/compute/pool_cpu/ops/reduce/tests.rs

### resize (7 files)
- pillow-rs/src/compute/pool_cpu/ops/resize.rs
- pillow-rs/src/compute/pool_cpu/ops/resize/filter.rs
- pillow-rs/src/compute/pool_cpu/ops/resize/float.rs
- pillow-rs/src/compute/pool_cpu/ops/resize/float_resize.rs
- pillow-rs/src/compute/pool_cpu/ops/resize/float_resize_tests.rs
- pillow-rs/src/compute/pool_cpu/ops/resize/integer.rs
- pillow-rs/src/compute/pool_cpu/ops/resize/tests.rs

### rotate (6 files)
- pillow-rs/src/compute/pool_cpu/ops/rotate.rs
- pillow-rs/src/compute/pool_cpu/ops/rotate/affine.rs
- pillow-rs/src/compute/pool_cpu/ops/rotate/affine/coordinates.rs
- pillow-rs/src/compute/pool_cpu/ops/rotate/cubic.rs
- pillow-rs/src/compute/pool_cpu/ops/rotate/scalar.rs
- pillow-rs/src/compute/pool_cpu/ops/rotate/tests.rs

### thumbnail (3 files)
- pillow-rs/src/compute/pool_cpu/ops/thumbnail.rs
- pillow-rs/src/compute/pool_cpu/ops/thumbnail/fusion.rs
- pillow-rs/src/compute/pool_cpu/ops/thumbnail/tests.rs

### transform (6 files)
- pillow-rs/src/compute/pool_cpu/ops/transform.rs
- pillow-rs/src/compute/pool_cpu/ops/transform/affine.rs
- pillow-rs/src/compute/pool_cpu/ops/transform/projective.rs
- pillow-rs/src/compute/pool_cpu/ops/transform/scalar.rs
- pillow-rs/src/compute/pool_cpu/ops/transform/scalar/projective.rs
- pillow-rs/src/compute/pool_cpu/ops/transform/tests.rs

### transpose (5 files)
- pillow-rs/src/compute/pool_cpu/ops/transpose.rs
- pillow-rs/src/compute/pool_cpu/ops/transpose/arch.rs
- pillow-rs/src/compute/pool_cpu/ops/transpose/kernels/transpose_rgba/x86_64.rs
- pillow-rs/src/compute/pool_cpu/ops/transpose/native.rs
- pillow-rs/src/compute/pool_cpu/ops/transpose/tests.rs

### SIMD source groups
### (pool support) (4 files)
- pillow-rs/src/compute/pool_simd/batch_execution.rs
- pillow-rs/src/compute/pool_simd/mod.rs
- pillow-rs/src/compute/pool_simd/point_batch.rs
- pillow-rs/src/compute/pool_simd/put_pixel_batch.rs

### alpha_composite.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/alpha_composite.rs

### box_blur (8 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur/kernels.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur/kernels/la.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur/kernels/luma.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur/kernels/rgb.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur/kernels/rgba.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur/kernels/rgba_avx2.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur/kernels/rgba_neon.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur/tests.rs

### box_blur.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur.rs

### chops (3 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/chops/kernels.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/chops/kernels/fused_multiply_screen.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/chops/kernels/tests.rs

### chops.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/chops.rs

### color3dlut.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/color3dlut.rs

### colorize.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/colorize.rs

### convert (13 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/alpha.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/cmyk.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/color_space.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/dispatch.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/dispatch/layout.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/dispatch/luma.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/dispatch/typed.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/layout.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/mod.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/rgba_rgb_neon.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/rgba_rgb_ssse3.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/tests.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/tests/cmyk_packing.rs

### convert.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/convert.rs

### crop.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/crop.rs

### draw (6 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/draw/kernels/mod.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/draw/kernels/shapes.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/draw/kernels/shapes/rounded_rect.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/draw/kernels/spans.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/draw/kernels/spans/line.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/draw/tests.rs

### draw.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/draw.rs

### duplicate (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/duplicate/tests.rs

### duplicate.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/duplicate.rs

### effects (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/effects/tests.rs

### effects.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/effects.rs

### enhance (3 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/enhance/point.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/enhance/sharpness.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/enhance/tests.rs

### enhance.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/enhance.rs

### expand (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/expand/tests.rs

### expand.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/expand.rs

### extract_band.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/extract_band.rs

### filter (10 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/byte_layouts.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/i32_three_by_three.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/l.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/la.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/mod.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/rgb_blur_ring.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/three_by_three.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/uniform_rgb.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/unsharp.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/tests.rs

### filter.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/filter.rs

### flip.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/flip.rs

### gaussian_blur (3 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/gaussian_blur/kernels/la.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/gaussian_blur/kernels/rgba_uniform.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/gaussian_blur/tests.rs

### gaussian_blur.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/gaussian_blur.rs

### gradient.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/gradient.rs

### grayscale (7 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/kernels/channels.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/kernels/cmyk.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/kernels/mod.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/kernels/rgb_avx2.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/kernels/ycbcr.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/kernels/ycbcr_ssse3.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/tests.rs

### grayscale.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale.rs

### histogram.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/histogram.rs

### layout.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/layout.rs

### lut (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/lut/tests.rs

### lut.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/lut.rs

### mandelbrot.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/mandelbrot.rs

### median_filter (4 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/median_filter/extrema.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/median_filter/float.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/median_filter/kernels.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/median_filter/tests.rs

### median_filter.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/median_filter.rs

### merge (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/merge/tests.rs

### merge.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/merge.rs

### module_blend.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/module_blend.rs

### native/shared: adapters (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters.rs

### native/shared: mod (1 files)
- pillow-rs/src/compute/pool_simd/ops/mod.rs

### native/shared: native_channel (2 files)
- pillow-rs/src/compute/pool_simd/ops/native_channel.rs
- pillow-rs/src/compute/pool_simd/ops/native_channel/cmyk_ssse3.rs

### native/shared: native_filter (1 files)
- pillow-rs/src/compute/pool_simd/ops/native_filter.rs

### native/shared: native_merge (1 files)
- pillow-rs/src/compute/pool_simd/ops/native_merge.rs

### native_arithmetic.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/native_arithmetic.rs

### native_bytes.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/native_bytes.rs

### neighborhood.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/neighborhood.rs

### offset (2 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/offset/kernels/byte.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/offset/kernels/luma16.rs

### offset.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/offset.rs

### pad (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/pad/tests.rs

### pad.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/pad.rs

### paste (17 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/apply.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/apply/cmyk_l.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/apply/generic.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/apply/l.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/apply/la.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/apply/pa.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/apply/rgb.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/apply/rgba.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/blend.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/luma.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/luma16.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/mod.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/rgb_cmyk.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/mod.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/plan.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/rgb_to_rgba.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/tests.rs

### pipeline.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/pipeline.rs

### projection.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/projection.rs

### put_alpha (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/put_alpha/kernels.rs

### put_alpha.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/put_alpha.rs

### put_data.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/put_data.rs

### put_pixel (2 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/put_pixel/kernels.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/put_pixel/kernels/rgba_batch.rs

### put_pixel.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/put_pixel.rs

### reduce (2 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/reduce/kernels.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/reduce/tests.rs

### reduce.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/reduce.rs

### resize (17 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/aspect.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/coordinates.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/f32.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/f32/boxed.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/i32.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/i32/neon.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/luma16.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/u8.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/u8_boxed_convolution.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/u8_convolution.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/u8_cover_i32.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/u8_nearest.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/u8_plan.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/support.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/support/aspect.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/support/resize.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/tests.rs

### resize.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/resize.rs

### rotate (8 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate/interpolation.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate/kernels.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate/kernels/native.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate/kernels/native/bilinear.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate/kernels/native/nearest.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate/kernels/native/typed_bilinear.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate/kernels/nearest.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate/tests.rs

### rotate.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate.rs

### support.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/support.rs

### thumbnail (3 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/thumbnail/kernels/float.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/thumbnail/kernels/fusion.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/thumbnail/kernels/integer.rs

### thumbnail.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/thumbnail.rs

### transform (10 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/kernels.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/kernels/bilinear.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/kernels/bilinear/affine.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/kernels/bilinear/projective.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/kernels/luma16.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/kernels/mesh.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/kernels/nearest.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/kernels/nearest/affine.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/kernels/nearest/projective.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/tests.rs

### transform.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/transform.rs

### transpose (5 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/transpose/kernels.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/transpose/kernels/portable.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/transpose/kernels/rgba_avx2.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/transpose/odd_tests.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/transpose/tests.rs

### transpose.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/transpose.rs


## 3. Mode and pixel-storage inventory

The raster-owned mode identity needs to be carried by the materialized DynamicImage variant itself. Do not infer it from channels, ColorType, buffer contents, or a separate string. Decode metadata from image-slash-star must be adapted at the raster boundary; it must not be added to image-slash-star's type. Lazy pipeline operations must use the source mode and explicitly account for mode-changing operations before materialization.

The current storage-level DynamicImage cases are not equivalent to the 20 Pillow modes. Source review must classify:
- all image construction and decode boundaries;
- raw-byte imports, frombuffer/fromarray-style adapters, and explicit conversion;
- mode-preserving crop, flip/transpose, resize, rotate, transform, and thumbnail;
- mode-changing conversion, LAB conversion, palette remap, grayscale, band extraction, alpha insertion, and other output-mode operations;
- secondary image, mask, merge-band, paste, and composite input identities;
- palette index/alpha metadata, premultiplied alpha, RGBX filler bytes, LAB conventions, I and F byte carriers, and I;16 byte order;
- all error/rejection outcomes and all high-depth representations.
A branch on a mode string or PixelMode in baseline code is a review lead, not a complete support contract.

### Literal mode candidates found in baseline source

These are strings found in production CPU/SIMD files, excluding test files. They are search evidence only; absence of a literal does not imply unsupported behavior, and a literal can be a comment, error, test helper, or validation route. Reconcile each candidate against the function body and public caller during migration.

CPU candidate modes by source family:
- (pool support): L, LA, RGB, RGBA, F
- color: 1, L, LA, La, P, RGB, RGBA, RGBa, RGBX, CMYK, I;16, I;16L, I;16B, I;16N
- composite: P, RGB, LAB
- draw: I;16, I;16B, I;16N, F
- enhance: L, LA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, HSV
- eval: 1, L, P
- filter: L, LA, RGB, RGBA, I, F
- grayscale: 1, L, La, RGBa, CMYK, YCbCr, HSV, I, F
- imageops: 1, L, LA, P, PA, RGB, RGBA, RGBX, CMYK, HSV, F
- paste: 1, L, LA, La, P, PA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, LAB, HSV, I;16, I;16L, I;16B, I;16N, I, F
- reduce: La, PA, RGB, RGBA, RGBa, RGBX, CMYK, I, F
- resize: 1, LA, La, RGBA, RGBa, I, F
- rotate: 1, L, LA, La, P, PA, RGBa, RGBX, CMYK, I;16, I;16B, I;16N, I, F
- thumbnail: 1, P, RGB, RGBa, RGBX, CMYK, I, F
- transform: 1, L, LA, La, P, PA, RGB, RGBa, RGBX, CMYK, YCbCr, HSV, I;16, I;16B, I;16N, I, F

SIMD candidate modes by source family:
- (pool support): L, LA, P, PA, RGB, RGBA, I;16, I;16L, I;16B, I;16N, I, F
- alpha_composite.rs: LA, RGBA
- box_blur.rs: L, RGB, RGBA
- chops.rs: 1, L, LA, P, PA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, HSV
- colorize.rs: L
- convert: LA, La, RGBA, RGBa
- convert.rs: 1, L, RGB, RGBA, RGBa, RGBX, CMYK, I;16, I;16L, I;16B, I;16N
- draw.rs: 1, L, LA, P, PA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, HSV, I, F
- effects.rs: 1, L, LA, P, PA, RGB, RGBA, CMYK
- enhance: L, LA, RGB, RGBA, RGBX, YCbCr, HSV
- enhance.rs: L, LA, RGB, RGBA, RGBX, CMYK, YCbCr, HSV
- extract_band.rs: 1, L, LA, P, PA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, HSV
- filter: L, LA
- filter.rs: 1, L, LA, PA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, HSV, I
- flip.rs: 1, L, LA, P, PA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, HSV, I, F
- gaussian_blur.rs: L, LA, RGB
- gradient.rs: 1, L, P, I, F
- grayscale.rs: L, LA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr
- histogram.rs: 1, L, RGB
- layout.rs: 1, L, LA, P, PA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, HSV, I;16, I;16L, I;16B, I;16N, I, F
- lut.rs: 1, L, LA, P, RGB, RGBA, CMYK, YCbCr, HSV
- median_filter: F
- median_filter.rs: LA, F
- merge.rs: L, LA, P, RGB, RGBA, CMYK
- module_blend.rs: L, LA, La, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, HSV
- offset: I;16, I;16L, I;16B, I;16N
- pad.rs: L, LA, P, RGB, RGBA, RGBX, CMYK, I, F
- paste: 1, L, LA, P, PA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, HSV, I;16, I;16L, I;16B, I;16N, I, F
- pipeline.rs: 1, L, LA, La, P, PA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, LAB, HSV, I;16, I, F
- put_alpha.rs: L, LA, P, PA, RGB, RGBA, RGBX, CMYK
- put_pixel.rs: RGB, RGBA, LAB
- reduce.rs: L, LA, RGB, RGBA, CMYK, YCbCr, HSV
- resize: 1, L, LA, P, PA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, HSV, I;16, I;16L, I;16B, I;16N, I, F
- resize.rs: LA, La, PA, RGBA, RGBa, RGBX, CMYK, I;16, I;16L, I;16B, I;16N, I, F
- rotate: PA, RGBA, I, F
- rotate.rs: 1, P, PA, RGBa, RGBX, CMYK, I;16, I;16L, I;16B, I;16N, I, F
- thumbnail: RGB
- thumbnail.rs: 1, P, RGB, RGBa, RGBX, CMYK, I, F
- transform: L, I;16, I;16B, I;16N
- transform.rs: L, LA, RGB, RGBA, CMYK, YCbCr, HSV, I;16, I;16L, I;16B, I;16N
- transpose.rs: 1, L, LA, P, PA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, HSV, I, F

## 4. CPU/SIMD architecture and parallel inventory

### Reduce family mode and architecture migration (source review complete; parity pending)

The new operation owner is present at pillow-rs/src/compute/pool_cpu/ops/reduce.
Its top-level ops.rs classifies every one of the 25 exact Mode identities,
assesses request factors, selects a declared CPU strategy, and prepares the
selected mode implementation. The CPU registry now calls that prepared entry
point. Every mode has a same-mode serial Default strategy using its native
carrier and returning an exact mode-bearing DynamicImage. L, LA, RGB, RGBA,
CMYK, YCbCr, and HSV also declare exact-mode AVX2 strategies for the archived
native-byte vector reducer. The shared AVX2 helper uses bounded AVX2 fixed-point
lanes, while the portable wide-lane helper is selected as SSE2, NEON, or wasm
SIMD128 when built and supported. F32 declares its archived ordered 2x2
portable wide-lane kernel as AVX or SSE2 when compiled for those features, and
NEON or wasm SIMD128 when built. Selection is guarded to even source dimensions
and eight complete output samples per row; other F32 reductions use its native
Default strategy. Each mode declaration supplies its native
channel and alpha semantics. RGB, YCbCr, and HSV retain the 3-byte 2x2 vector
block; RGB retains the 3x5 fast block and sparse 2x2 path. Both vector kernels
retain premultiplication and per-edge rounding rules, while partial row tails
use the same mode's Default helper. The special 3-byte blocks require eight
complete source blocks; incomplete right edges use the bounded general path.
Correctness evidence for these routes belongs in the mode-by-architecture
parity matrix; no unit tests are being added for Reduce. Expand parity coverage
to exercise each supported mode against every available architecture and its
Default fallback, including edges, vector tails, and sparse RGB inputs.

| Exact source identity | Maintained strategy |
| --- | --- |
| One, ImageLuma8 | Native one-byte reducer with mode-preserving output; ImageLuma8 declares AVX2 and portable SSE2/NEON/wasm SIMD lane strategies. |
| ImageLumaA8 | Native LA bytes with archived premultiply/average/unpremultiply arithmetic; declares AVX2 and portable SSE2/NEON/wasm SIMD lane strategies. |
| ImageLaPremultiplied8 | Native LA bytes averaged without straight-alpha premultiplication. |
| ImagePalette8 | Explicit identity-only strategy; actual P reduction is rejected by the factor contract, matching the public Pillow contract. |
| ImagePaletteAlpha8 | Native two-byte index/alpha reduction with no alpha premultiplication or palette conversion; Default only, matching the archived CPU path. |
| ImageRgb8 | Native RGB reducer retaining the archived dense-even and sparse 2x2 paths plus the general RGB row kernel; AVX2 and portable SSE2/NEON/wasm SIMD handle lane reduction, including the 2x2 and 3x5 blocks. |
| ImageRgba8 | Native RGBA reducer retaining premultiplied-alpha arithmetic and its specialized block sum; declares AVX2 and portable SSE2/NEON/wasm SIMD lane strategies. |
| ImageRgbPremultiplied8, ImageRgbx8 | Four-byte native strategies with mode-specific alpha/channel semantics and no cross-mode conversion. |
| ImageCmyk8 | Four-byte native samples with no alpha semantics; declares AVX2 and portable SSE2/NEON/wasm SIMD lane strategies. |
| ImageYCbCr8, ImageHsv8 | Three-byte native reducers; samples are averaged without color-space transforms and declare AVX2 plus portable SSE2/NEON/wasm SIMD lane strategies. |
| ImageLab8 | Three-byte native reducer; samples are averaged without color-space transforms. |
| ImageI32 | Scalar four-byte carrier with the archived I wrapping arithmetic, returned under its exact mode variant. The fused even 2x2 Thumbnail stream has its own mode/architecture owners; general Thumbnail reduction has no vector arithmetic. |
| ImageF32 | Scalar four-byte carrier with archived F ordered arithmetic and exact mode output; eligible even 2x2 reductions declare AVX, SSE2, NEON, and Wasm SIMD128 strategies, including the nested Thumbnail reducing stage. |
| ImageI16, ImageI16L, ImageI16B, ImageI16N | Native typed u16 reduction that preserves each exact I;16 identity. This is core-only and marked NO_PILLOW_SUPPORT. |
| ImageLumaA16, ImageRgb16, ImageRgba16, ImageRgb32F, ImageRgba32F | Native typed high-depth reduction that preserves each internal identity. These are core-only and marked NO_PILLOW_SUPPORT. |

Thumbnail's non-fused reducing stage reuses the prepared Reduce owner, including
its I/F scalar helpers; RGB PutPixel/Thumbnail fusion calls the shared sparse-
row reducer. Caller review covers the public `reduce_public` validation and
lazy pipeline entry, the prepared registry route, exact-mode result execution,
and Thumbnail's nested Reduce plan using the same injected `ExecutionContext`.
P's identity-only contract and PA's native two-byte behavior agree with the
public wrapper and archived CPU path. The only remaining Reduce-specific source
migration is archived Rayon scheduling for the later `pool_parallel` backend;
`pool_cpu` remains serial. Mode-by-architecture parity is still pending, and no
unit tests are being added for this family. No build, formatting, lint, parity,
or benchmark verification has run. The archived SIMD instructions and guards
are in `pool_cpu/legacy/pool_simd/ops/adapters/reduce.rs` and its
`reduce/kernels` files.

Architecture evidence below records baseline target-feature annotations and runtime probes. The real instruction set is determined by the implementation body and its safety contract, not by a filename alone. Move each true specialization to its exact architecture directory, retain the required compile-time gates, and route through injected Architecture only after the centralized capability policy guarantees execution safety. Portable shared kernels remain in Default or the narrow shared-helper location; do not create empty architecture folders.

CPU target-feature/probe sites:
- ops/filter/filter3x3.rs:473: target_feature = "neon"
- ops/filter/filter5x5.rs:220: #[cfg(target_feature = "neon")]
- ops/filter/filter5x5/int32.rs:36: std::is_x86_feature_detected!("fma").then_some(Self { _private: () })

SIMD target-feature/probe sites:
- ops/adapters.rs:49: target_feature = "neon"
- ops/adapters/box_blur.rs:220: std::is_x86_feature_detected!("avx2")
- ops/adapters/box_blur/kernels/rgba_avx2.rs:14: #[target_feature(enable = "avx2")]
- ops/adapters/box_blur/kernels/rgba_avx2.rs:95: #[target_feature(enable = "avx2")]
- ops/adapters/box_blur/kernels/rgba_avx2.rs:161: if !std::is_x86_feature_detected!("avx2") {
- ops/adapters/convert/cmyk.rs:3: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/convert/cmyk.rs:6: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/convert/cmyk.rs:8: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/convert/cmyk.rs:10: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/convert/cmyk.rs:22: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/convert/cmyk.rs:26: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/convert/cmyk.rs:32: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/convert/cmyk.rs:66: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/convert/cmyk.rs:78: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/convert/cmyk.rs:107: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/convert/cmyk.rs:116: let ssse3_available = std::is_x86_feature_detected!("ssse3");
- ops/adapters/convert/kernels/layout.rs:8: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/convert/kernels/layout.rs:10: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/convert/kernels/layout.rs:135: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/convert/kernels/layout.rs:379: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/convert/kernels/layout.rs:417: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/convert/kernels/layout.rs:433: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/convert/kernels/layout.rs:453: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/convert/kernels/layout.rs:484: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/convert/kernels/mod.rs:6: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/convert/kernels/mod.rs:30: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/convert/kernels/rgba_rgb_ssse3.rs:18: || !std::is_x86_feature_detected!("ssse3")
- ops/adapters/convert/kernels/rgba_rgb_ssse3.rs:30: #[target_feature(enable = "ssse3")]
- ops/adapters/convert/kernels/rgba_rgb_ssse3.rs:99: if !std::is_x86_feature_detected!("ssse3") {
- ops/adapters/duplicate.rs:3: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/duplicate.rs:10: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/duplicate.rs:12: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/duplicate.rs:31: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/duplicate.rs:44: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/duplicate.rs:94: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/duplicate.rs:96: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/duplicate.rs:108: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/duplicate.rs:110: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/enhance/sharpness.rs:274: target_feature = "fma",
- ops/adapters/enhance/sharpness.rs:275: all(target_arch = "aarch64", target_feature = "neon")
- ops/adapters/enhance/sharpness.rs:281: target_feature = "fma",
- ops/adapters/enhance/sharpness.rs:282: all(target_arch = "aarch64", target_feature = "neon")
- ops/adapters/extract_band.rs:73: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/extract_band.rs:85: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/extract_band.rs:300: if !std::is_x86_feature_detected!("ssse3") {
- ops/adapters/extract_band.rs:337: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/extract_band.rs:373: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/filter/kernels/three_by_three.rs:4: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/filter/kernels/three_by_three.rs:311: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/filter/kernels/three_by_three.rs:390: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/grayscale/kernels/channels.rs:10: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/grayscale/kernels/channels.rs:61: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/grayscale/kernels/channels.rs:76: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/grayscale/kernels/channels.rs:111: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/grayscale/kernels/channels.rs:212: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/grayscale/kernels/channels.rs:218: all(target_arch = "aarch64", target_feature = "neon")
- ops/adapters/grayscale/kernels/cmyk.rs:5: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/grayscale/kernels/cmyk.rs:7: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/grayscale/kernels/cmyk.rs:10: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/grayscale/kernels/cmyk.rs:12: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/grayscale/kernels/cmyk.rs:14: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/grayscale/kernels/cmyk.rs:20: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/grayscale/kernels/cmyk.rs:36: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/grayscale/kernels/cmyk.rs:62: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/grayscale/kernels/cmyk.rs:80: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/grayscale/kernels/cmyk.rs:91: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/grayscale/kernels/rgb_avx2.rs:11: if !std::is_x86_feature_detected!("avx2") {
- ops/adapters/grayscale/kernels/rgb_avx2.rs:120: #[target_feature(enable = "avx2")]
- ops/adapters/grayscale/kernels/rgb_avx2.rs:178: if !std::is_x86_feature_detected!("avx2") {
- ops/adapters/grayscale/kernels/ycbcr.rs:24: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/grayscale/kernels/ycbcr.rs:74: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/grayscale/kernels/ycbcr.rs:79: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/grayscale/kernels/ycbcr.rs:85: if std::is_x86_feature_detected!("ssse3") {
- ops/adapters/grayscale/kernels/ycbcr.rs:92: all(target_arch = "aarch64", target_feature = "neon"),
- ops/adapters/grayscale/kernels/ycbcr.rs:122: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/grayscale/kernels/ycbcr.rs:136: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/grayscale/kernels/ycbcr_ssse3.rs:11: if source.len() % 3 != 0 || !std::is_x86_feature_detected!("ssse3") {
- ops/adapters/grayscale/kernels/ycbcr_ssse3.rs:20: #[target_feature(enable = "ssse3")]
- ops/adapters/lut.rs:380: target_feature = "neon",
- ops/adapters/lut.rs:428: target_feature = "neon",
- ops/adapters/lut.rs:530: target_feature = "neon",
- ops/adapters/lut.rs:546: target_feature = "neon",
- ops/adapters/lut.rs:679: target_feature = "neon",
- ops/adapters/merge.rs:140: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/merge.rs:148: if channels == 3 && pixels >= 16 && std::is_x86_feature_detected!("ssse3") {
- ops/adapters/module_blend.rs:174: all(target_arch = "aarch64", target_feature = "neon"),
- ops/adapters/module_blend.rs:177: target_feature = "fma"
- ops/adapters/paste/kernels/luma.rs:18: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/paste/kernels/luma.rs:119: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/paste/kernels/luma.rs:121: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/paste/kernels/luma.rs:123: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/paste/kernels/luma.rs:204: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/paste/kernels/luma.rs:270: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/paste/kernels/luma.rs:325: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/paste/kernels/luma.rs:353: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/paste/kernels/luma.rs:400: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/paste/kernels/luma.rs:434: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/paste/kernels/luma.rs:436: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/paste/kernels/rgb_cmyk.rs:107: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/paste/kernels/rgb_cmyk.rs:196: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/paste/kernels/rgb_cmyk.rs:205: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/paste/kernels/rgb_cmyk.rs:208: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/paste/kernels/rgb_cmyk.rs:358: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/paste/kernels/rgb_cmyk.rs:367: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/paste/kernels/rgb_cmyk.rs:370: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/paste/kernels/rgb_cmyk.rs:459: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:21: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:30: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:32: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:38: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:58: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:81: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:103: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:126: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:134: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:136: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:142: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:296: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:300: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/resize/kernels/i32.rs:339: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:363: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:369: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/resize/kernels/i32.rs:382: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:412: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:414: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/resize/kernels/i32.rs:427: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:446: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:452: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/resize/kernels/i32.rs:455: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:500: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:502: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/resize/kernels/i32.rs:517: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:523: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/resize/kernels/i32/neon.rs:50: // by the module's compile-time target_feature gate; the four accumulators
- ops/adapters/resize/kernels/i32/neon.rs:129: // AArch64 NEON is enabled by the module's compile-time target_feature gate.
- ops/adapters/rotate/interpolation.rs:61: target_feature = "fma",
- ops/adapters/rotate/interpolation.rs:62: all(target_arch = "aarch64", target_feature = "neon")
- ops/adapters/rotate/interpolation.rs:68: target_feature = "fma",
- ops/adapters/rotate/interpolation.rs:69: all(target_arch = "aarch64", target_feature = "neon")
- ops/adapters/thumbnail/kernels/float.rs:75: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/thumbnail/kernels/float.rs:254: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/thumbnail/kernels/float.rs:273: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/thumbnail/kernels/float.rs:298: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/transpose/kernels/rgba_avx2.rs:27: || !std::is_x86_feature_detected!("avx2")
- ops/adapters/transpose/kernels/rgba_avx2.rs:95: #[target_feature(enable = "avx2")]
- ops/native_channel.rs:9: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/native_channel.rs:11: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/native_channel.rs:14: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/native_channel.rs:70: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/native_channel.rs:74: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/native_channel.rs:78: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/native_channel.rs:98: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/native_channel.rs:102: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/native_channel.rs:106: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/native_channel.rs:130: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/native_channel.rs:177: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/native_channel.rs:189: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/native_channel.rs:233: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/native_channel.rs:244: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/native_channel.rs:280: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/native_channel.rs:317: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/native_channel/cmyk_ssse3.rs:30: #[target_feature(enable = "ssse3")]
- ops/native_channel/cmyk_ssse3.rs:69: if !std::is_x86_feature_detected!("ssse3") {
- ops/native_merge.rs:8: all(target_arch = "aarch64", target_feature = "neon")
- ops/native_merge.rs:13: all(target_arch = "aarch64", target_feature = "neon")
- ops/native_merge.rs:66: #[target_feature(enable = "ssse3")]
- ops/native_merge.rs:134: #[target_feature(enable = "ssse3")]
- ops/native_merge.rs:233: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/native_merge.rs:361: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]

Architecture filename candidates in pool_simd include: rgba_avx2 box blur, NEON and SSSE3 RGBA-to-RGB conversion, AVX2 RGB grayscale, SSSE3 YCbCr grayscale, NEON integer resize, AVX2 RGBA transpose, and SSSE3 CMYK channel projection. Source review must also classify portable channel kernels and FMA-sensitive paths that are not named after an architecture.

Parallel/Rayon source that must be removed from maintained CPU and assigned to the later Parallel CPU work only after its own stage:
- mod.rs: lines 213, 230
- ops/chops.rs: lines 9, 111, 158
- ops/composite.rs: lines 162, 239, 256
- ops/crop.rs: lines 49
- ops/effects.rs: lines 10, 26, 36
- ops/enhance.rs: lines 7, 23, 33
- ops/filter.rs: lines 37, 39
- ops/filter/blur.rs: lines 14, 16, 19, 23, 29, 60, 79, 90, 101, 123, 152, 214, 220, 222, 226, 230, 243, 246, 259, 330, 360, 405, 481, 497, 523, 525, 527, 529, 531, 533, 535, 537
- ops/filter/blur/box_rows.rs: lines 3, 5, 12, 55, 95, 183, 263
- ops/filter/blur/line.rs: lines 185, 188, 266
- ops/filter/blur/radius_one.rs: lines 4, 7, 11, 18, 35, 126, 221, 324, 431, 514, 561
- ops/filter/blur/radius_one/vertical.rs: lines 5, 69, 120, 124, 151, 180
- ops/filter/filter3x3.rs: lines 11, 206, 230, 312, 382, 400, 471, 495
- ops/filter/filter5x5.rs: lines 15, 18, 21, 26, 29, 32, 34, 68, 77, 91, 103, 123, 136, 194, 218, 242
- ops/filter/filter5x5/int32.rs: lines 7, 12, 17, 20, 28, 33, 40, 62, 76, 153, 158, 171, 189, 215, 297, 398, 467, 619, 638, 705, 712, 721, 745
- ops/filter/filter5x5/int32/bounded_binomial.rs: lines 1, 26, 55, 79
- ops/filter/filter5x5/native_bytes.rs: lines 4, 6, 90, 100, 116, 134, 202, 236, 265, 427, 512, 516
- ops/filter/filter5x5/native_bytes/binomial.rs: lines 16, 108, 210, 272
- ops/filter/rank.rs: lines 174, 192, 208, 224, 308, 317, 381, 390, 508, 512, 581, 590
- ops/filter/rank/float.rs: lines 57, 92, 222, 224
- ops/imageops.rs: lines 30, 36
- ops/imageops/expand.rs: lines 234, 243, 276
- ops/imageops/pad.rs: lines 32, 184, 454, 469, 494, 525, 557
- ops/imageops/pad/native.rs: lines 271, 286, 580, 593, 613
- ops/imageops/pixel.rs: lines 35, 37, 45, 47, 67, 93, 123, 404, 417, 426
- ops/paste.rs: lines 8, 83, 109
- ops/paste/kernels.rs: lines 3, 154, 181
- ops/reduce.rs: lines 582, 609
- ops/reduce/kernels/rgb.rs: lines 113, 134, 185, 204
- ops/resize/float.rs: lines 18, 158, 276, 294, 357, 379, 428, 448, 470, 497, 687, 697, 708
- ops/resize/float_resize.rs: lines 3, 5, 7, 9, 13, 38, 64, 141, 153
- ops/resize/integer.rs: lines 4, 47, 68, 85, 92, 114, 170, 228, 306, 321, 336, 348, 368, 372, 398, 400, 434, 437, 497, 500, 512
- ops/rotate/affine.rs: lines 6, 18, 61, 82, 103, 106, 127, 203, 305
- ops/rotate/affine/coordinates.rs: lines 1, 14, 38, 79, 86
- ops/thumbnail.rs: lines 9, 19, 28, 70, 210, 462, 480
- ops/transpose.rs: lines 11, 14, 20, 50, 91, 101, 179, 198
- ops/transpose/arch.rs: lines 6
- ops/transpose/native.rs: lines 83, 95, 97
Parallel/Rayon source evidence in the preserved SIMD implementation:
- ops/adapters.rs: lines 47, 179, 361
- ops/adapters/alpha_composite.rs: lines 419, 434
- ops/adapters/box_blur/kernels.rs: lines 3, 5, 7, 13, 413, 444, 479, 498, 526, 548, 556, 579, 582, 586
- ops/adapters/chops.rs: lines 15, 643, 664
- ops/adapters/chops/kernels.rs: lines 3, 80, 91, 140, 151, 476, 492
- ops/adapters/convert.rs: lines 25
- ops/adapters/convert/kernels/layout.rs: lines 497
- ops/adapters/convert/kernels/mod.rs: lines 15
- ops/adapters/enhance/sharpness.rs: lines 245, 254
- ops/adapters/filter.rs: lines 505, 707
- ops/adapters/filter/kernels/byte_layouts.rs: lines 3, 192, 199
- ops/adapters/filter/kernels/i32_three_by_three.rs: lines 165, 177
- ops/adapters/filter/kernels/mod.rs: lines 5, 38
- ops/adapters/filter/kernels/three_by_three.rs: lines 606, 618, 719, 731
- ops/adapters/grayscale/kernels/cmyk.rs: lines 3, 126, 142, 158
- ops/adapters/lut.rs: lines 3, 386, 405
- ops/adapters/median_filter.rs: lines 606, 628
- ops/adapters/median_filter/extrema.rs: lines 219, 236, 331, 342
- ops/adapters/median_filter/float.rs: lines 7, 201, 230
- ops/adapters/native_bytes.rs: lines 242, 244, 259
- ops/adapters/paste/kernels/apply.rs: lines 19
- ops/adapters/paste/kernels/apply/l.rs: lines 4, 57, 78
- ops/adapters/paste/kernels/apply/la.rs: lines 4, 64, 85
- ops/adapters/paste/kernels/apply/pa.rs: lines 4, 63, 84
- ops/adapters/paste/kernels/apply/rgb.rs: lines 4, 84, 105
- ops/adapters/paste/rgb_to_rgba.rs: lines 3, 84, 109
- ops/adapters/put_alpha.rs: lines 11, 395, 419, 475
- ops/adapters/put_alpha/kernels.rs: lines 3, 314, 332
- ops/adapters/reduce.rs: lines 10, 52, 494, 537
- ops/adapters/resize/kernels/u8.rs: lines 3, 5, 20, 225, 231, 266, 268, 270, 280, 289, 301, 331, 425, 429, 436, 461, 555
- ops/adapters/resize/kernels/u8_boxed_convolution.rs: lines 3, 13, 252, 271, 311, 319, 351, 444, 452, 480
- ops/adapters/rotate.rs: lines 6
- ops/adapters/rotate/kernels.rs: lines 4, 11
- ops/adapters/rotate/kernels/native/nearest.rs: lines 74
- ops/adapters/transpose.rs: lines 5, 13, 18, 85, 222, 224, 313
- ops/adapters/transpose/kernels.rs: lines 9, 14
- ops/adapters/transpose/kernels/portable.rs: lines 3, 5, 7, 9, 13, 18, 258, 273, 286, 310, 317, 339, 348, 408, 418, 470, 497, 505, 529, 540, 599, 612
The archive preserves these paths unchanged; no active CPU executor may retain feature-based scheduling or parallel flags.

## 5. Registry, context, eager, GPU, and wrapper callsites

At the baseline inventory freeze, Backend contained Cpu, Gpu, and Simd;
registry entries held separate CpuOpFn and SimdOpFn pointers; backend_state
built independent CPU/GPU/SIMD pools; and CPU/SIMD leaf signatures accepted
`Option<&str>` mode. This is baseline evidence, not a claim about the current
partial migration.

Baseline direct production references from compute modules to
pool_cpu/pool_simd and their function-pointer interfaces:
- pillow-rs/src/compute/automatic_segments.rs: 1 references at lines 3
- pillow-rs/src/compute/eager.rs: 7 references at lines 92, 101, 107, 502, 531, 573, 621
- pillow-rs/src/compute/execution.rs: 6 references at lines 315, 355, 508, 610, 706, 715
- pillow-rs/src/compute/mod.rs: 4 references at lines 331, 332, 336, 339
- pillow-rs/src/compute/pool_gpu/auxiliary_images.rs: 6 references at lines 46, 148, 168, 296, 298, 309
- pillow-rs/src/compute/pool_gpu/batch_execution/routing.rs: 1 references at lines 126
- pillow-rs/src/compute/pool_gpu/batch_normalization.rs: 1 references at lines 249
- pillow-rs/src/compute/pool_gpu/batch_operation_planning.rs: 2 references at lines 204, 273
- pillow-rs/src/compute/pool_gpu/fusion.rs: 1 references at lines 58
- pillow-rs/src/compute/pool_gpu/logical_mode_admission.rs: 1 references at lines 115
- pillow-rs/src/compute/pool_gpu/mode.rs: 1 references at lines 252
- pillow-rs/src/compute/pool_gpu/native_input.rs: 1 references at lines 331
- pillow-rs/src/compute/pool_gpu/point_ops.rs: 1 references at lines 83
- pillow-rs/src/compute/pool_gpu/stream/graph/layout.rs: 1 references at lines 321
- pillow-rs/src/compute/registry.rs: 5 references at lines 54, 58, 66, 74, 80
- pillow-rs/src/compute/registry/registration/imageops.rs: 1 references at lines 151
- pillow-rs/src/compute/registry/registration/mod.rs: 4 references at lines 3, 13, 27, 37
- pillow-rs/src/compute/registry/registration/prelude.rs: 9 references at lines 4, 11, 14, 18, 23, 26, 31, 35, 40
- pillow-rs/src/compute/thumbnail_fusion.rs: 2 references at lines 14, 31

Context and routing files to audit as one connected graph:
- pillow-rs/src/compute/mod.rs: Backend and BackendImpl definitions; active pools and public dispatch exports.
- pillow-rs/src/compute/backend_state.rs: enabled backends, priority and pool construction.
- pillow-rs/src/compute/registry.rs and pillow-rs/src/compute/registry/registration/: operation function types, entries, registration and family exports.
- pillow-rs/src/compute/execution.rs, pillow-rs/src/compute/automatic_segments.rs, and pillow-rs/src/compute/execution/terminals.rs: selection, capability/admission, segment execution and terminal materialization.
- pillow-rs/src/compute/eager.rs: direct point, projection, enhancement, and gradient/generator routes.
- pillow-rs/src/compute/pool_cpu/batch_execution.rs and the prepared operation owners: mode flow and nested/fused CPU work. The generic `compute/thumbnail_fusion.rs` module has been removed.
- pillow-rs/src/pipeline.rs, pillow-rs/src/image.rs, and raster constructors: exact mode identity, planned transitions and materialized results.
- GPU code that imports CPU helpers: preserve GPU ownership and adapt only interfaces/type compatibility required by the shared raster/context change. Do not route GPU execution to CPU parallel scheduling.
- Python/JS bindings and decode adapters: preserve existing host validation and keep Pillow-owned metadata out of image-slash-star.
- All first-party tests that call CPU/SIMD leaf functions may need mechanical API rewiring after active code changes; do not change test cases, expected values, or assertions.

### CPU architecture routing status (source scan: 2026-10-31)

The following routing work is source-complete but unverified. It does not mark
CPU mode/kernel migration or the full source review complete.

| Owner | Status or remaining migration |
| --- | --- |
| `compute/mod.rs`, `compute/backend_state.rs`, `compute/registry.rs` | Source-complete: the active `SimdPool`, adapter registry, and pool exports are removed. The public compatibility alias routes through CPU plus Architecture; actual executor telemetry is CPU. |
| `compute/automatic_segments.rs`, `compute/execution.rs` | Source-complete: mixed CPU/SIMD segmentation, SIMD mode-transition guesses, adapter support scans, and retry routing are removed. Pipeline preparation now selects CPU plus the injected architecture context. |
| `compute/execution/terminals.rs`, `image/projection/putpixel_pipeline.rs` | The fused RGB/CMYK projection terminal executes CPU mode/architecture strategies and reuses its prepared context. GPU terminal admission remains separate and has an actionable TODO at its owner. |
| `compute/registry/registration/prelude.rs`, `compute/registry/registration/simd.rs` | Source-complete: the active SIMD adapter import and registration module are removed. Review each archived kernel against its CPU mode/architecture strategy before CPU source completion. |
| `compute/eager.rs`, `image/projection.rs`, `ops/module_fns.rs` | Eager RGB/CMYK GetProjection, LinearGradient, RadialGradient, and EffectMandelbrot use operation-owned CPU strategies and prepared receipts. Tiny-RGB eager ExtractBand now uses the prepared architecture executor for both serial CPU and compatibility SIMD requests, retaining the selected strategy in telemetry. The generic receipt helper remains test-only. Continue auditing other direct eager routes and context injection. |
| `compute/thumbnail_fusion.rs`, `image/pipeline_admission.rs` | The generic fusion module was removed. RGB PutPixel/Thumbnail admission and execution now use the CPU family's prepared mode/architecture contract; the fused resize retains the selected RGB Resize or ResizeBoxed strategy and sparse active-row facts. |
| `compute/pool_gpu/fusion.rs` | Resolved: adjacent transpose composition uses the shared `pipeline::TransposeTransform` algebra. |
| `image/channel_access.rs`, `image/data_access/get.rs` | Eager ExtractBand requests use prepared CPU mode/architecture strategies; Tiny-RGB getchannel now records the selected strategy for CPU Default and compatibility SIMD. GetData retains its direct caller-owned output slice. A string-based GPU admission table remains under an actionable TODO. |

## 6. Frozen verification scope

### CPU parity

Authority: pillow-rs/tests/fixtures/manifest.yaml, SHA-256 2c07ff46a18e90530a948273b8b7aca2706ea3a7d3bb7689a668aacd427f7fa4.
- Oracle: Pillow 12.2.0 on CPython 3.12.
- CPU target profile: python-cpu with pillow-rs-py/default and pillow-rs/default; no parallel feature.
- Input set: all 27 files named by input_index.parity, with no case or surface filters.
- Total manifest cases: 17408. CPU-applicable denominator: 17407.
- The sole non-CPU case excluded by the python-cpu profile is PIL.GpuBatchExecutor.submit.fault-contract.device-failure-at-admission. It belongs to the GPU fault-contract profile.
- Canonical sorted CPU case IDs are SHA-256 8da7b964ad255904d23d15df85f29135f2bf1f9647038330429a9972acde721b. The all-case sorted ID hash is c2df09d7dbd4861621ca99ac4687638163038e3e20ce2fcb9ac70bebdf5944ae. The sorted set is derived from the pinned manifest inputs; it is not a sampled list.
- Source/build identity and execution receipts must be generated only after the complete source migration. Full CPU parity command is recorded in Stage 7 of CPU_REFACTOR_PLAN.md. No parity was run for this inventory.

### CPU benchmarks

Authority: all 28 files named by input_index.benchmark in the same manifest.
- Workload count: 1032 unique workloads with python-cpu subjects; no workload filtering.
- Sorted workload ID SHA-256: 639e358082276c924343618d5a3b09c6945de012c91c1a80ac5a8ee2121ccb84.
- Benchmark input bundle SHA-256: 61753fbdf807e62b08579e65ee272daf3ca3425a6d2347ec95dee299f2d33e77, computed from sorted path and file-digest pairs.
- Freeze the existing measurement contract from each workload: samples, warmups, iterations, cache state, materialization boundary and parity gate remain unchanged.
- No new baseline campaign has been run for this inventory. Preserve compatible baseline evidence if it exists; otherwise measure the preserved baseline only after CPU static quality/compilation and parity pass, as required by the plan.
- The quiet repeat allowance and per-operation acceptance limits are in Stage 8 of CPU_REFACTOR_PLAN.md. Keep each mode/workload separately attributable to CPU Default or its selected CPU architecture.

## 7. Completion bookkeeping

Before any validation command, update this ledger with exact baseline-source to maintained-function paths, reviewed mode/output behavior, each architecture's instruction requirements and fallback, and every direct/eager caller rewire. Mark source review complete only when all 98 CPU files, all 182 SIMD files, all 84 pipeline variants, all direct callsites, and all supported/rejected mode paths are accounted for. Then proceed to the ordered static, CPU parity, and CPU performance gates in CPU_REFACTOR_PLAN.md.

### Maintained source progress: incomplete and unvalidated

### Composite-family source progress: operation strategies drafted; source review and parity pending

- The archived CPU implementation is
  `pool_cpu/legacy/ops/composite.rs`. `AlphaComposite` remains in the existing
  LA and RGBA strategies. Merge, BlendModule, and CompositeModule have
  operation-owned prepared plans and their registry entries point at those
  plans. The family `ops.rs` facade exposes support and preparation while
  `ops/{merge,blend,composite_module}.rs` keeps each contract beside its
  operation entry point. Exact mode strategies live under
  `composite/<mode>/default`, with shared native-sample helpers under
  `composite/helpers/`. These additions are source-only and unverified.
- `Merge` target contracts from `ops/module_fns.rs::validate_merge_shape` and
  `validate_merge_band_modes`:
  - `1`, `L`, `P`, `I`, `F`, and all four `I;16*` identities accept one band of
    the exact same mode and return that native raster unchanged. Internal
    high-depth identities are rejected with `ValueError("image has wrong
    mode")`.
  - `LA`, `La`, and `PA` accept two `L` bands, with a `P` first band allowed
    only in the multi-band position rule; the output is exact LA storage and
    the requested alias stays on the result identity.
  - `RGB`, `YCbCr`, `LAB`, and `HSV` accept three `L` bands, with the same
    first-`P` allowance; `LAB` applies wrapping `+128` to its second and third
    samples. The output is exact RGB-family storage and target identity.
  - `RGBA`, `RGBa`, `RGBX`, and `CMYK` accept four `L` bands, with the same
    first-`P` allowance; the four bytes are interleaved unchanged and the
    exact target identity is preserved. Band count, band identity, and size
    errors remain eager in the Rust wrapper.
- The archived serial RGB fast path avoids cloning each band and directly
  interleaves three native L slices. Other byte merge routes currently build
  temporary luma vectors, and `apply_effect_rows` can schedule Rayon when the
  parallel feature is enabled. The CPU Default implementation must remain
  serial; retain the Rayon branch only as reference for the later
  `pool_parallel` owner. Do not preserve the old `to_luma8` conversions where
  validated one-byte L/P samples can be read natively.
- The archived SIMD adapter is
  `legacy/pool_simd/ops/adapters/merge.rs`, with kernels in
  `legacy/pool_simd/ops/native_merge.rs`. Its exact native merge admission is
  limited to canonical L/LA/RGB/RGBA/CMYK targets and native one-byte L
  bands. It has a portable `wide::u8x16` block/tail interleaver for one through
  four channels, plus dedicated 16-pixel RGB SSSE3 and NEON kernels selected
  for sufficiently large inputs. The typed RGB support guard now covers the
  P-first admission and dimensions; those two architecture kernels preserve
  the archived vector/tail accounting and path telemetry. The active CPU
  owner also restores the portable wide routes; execution comparison remains
  pending before retiring the adapter. There is no active `pool_simd` pool.
- Source migration (2026-10-31): RGB's original planar SSSE3 and AArch64
  NEON interleavers live under `composite/rgb8/{ssse3,neon}` and remain guarded
  by the 16-pixel minimum, exact RGB target, L/P first band, remaining-L bands,
  equal dimensions, and required architecture. The archived portable
  `wide::u8x16` shuffle now has exact LA, RGB, RGBA, and CMYK mode entries for
  SSE2, NEON, and Wasm SIMD128. LA uses eight pixels per block; RGB, RGBA, and
  CMYK use four. RGB keeps the explicit SSSE3/NEON path first and uses the wide
  shuffle for smaller complete blocks. Every path preserves its exact output
  identity, has a scalar tail, records allocation/path/vector/tail telemetry,
  and falls back to same-mode serial Default when no complete block is
  eligible. No Merge unit tests are being added; parity will cover each mode
  and architecture, including block and scalar-tail boundaries. This
  migration is source-only and unverified.
- `BlendModule` accepts exact same-mode L pairs; LA/La pairs; RGB/LAB/HSV/YCbCr
  group pairs; and RGBA/CMYK/RGBa/RGBX group pairs. It rejects all other modes
  with the existing type-family errors. It blends every stored byte, including
  alpha and CMYK K, using the existing f32 fused arithmetic. LAB operands are
  un-biased before blending and the result is biased only when image1 is LAB.
  Serial Default no longer schedules Rayon. Exact native-byte modes declare
  AVX/FMA and AArch64 NEON strategies backed by shared 16-byte kernels; LAB
  remains on its bias-aware Default path. The admission guard preserves the
  finite-alpha, matching-dimensions, native-length, and empty-or-at-least-eight
  byte conditions, with same-mode Default fallback. Every accepted mode pair,
  architecture route, and fallback still needs parity comparison.
- `CompositeModule` returns image2's canvas and blends image1 through the
  admitted mask over the top-left intersection. `1`/`L` masks supply luma;
  `LA`/`RGBA`/`RGBa` masks supply alpha. The palette path blends indices and
  relies on the Pillow wrapper to preserve image2's palette. The maintained
  plan uses exact output identity and native byte, u16, i32, and f32 sample
  paths; the public source-to-destination mode conversion remains at the
  wrapper boundary. The non-Pillow high-depth formulas remain source-review
  items and do not claim Pillow parity. One, L, P, LA, PA, RGB, HSV, YCbCr,
  RGBA, CMYK, and RGBX declare SSE2, NEON, and Wasm SIMD128 masked-byte paths
  over the existing wide kernel with same-mode Default fallback. Their guard
  requires exact source/destination mode, valid native lengths, and a matching
  mask shape and selector. Mask samples are borrowed from native storage; mode
  `1` is treated as a binary mask. Parity must cover every exact mode, mask
  selector, architecture, block/tail boundary, and fallback.
- `AlphaComposite` remains restricted to exact LA and RGBA inputs, preserves
  Pillow's fixed-point pixel arithmetic and existing ignored-offset behavior,
  and has maintained SSE2/NEON/Wasm SIMD128 routes under exact LA/RGBA
  architecture folders. Those small wrappers reuse one shared wide kernel in
  the family helpers. Its full-frame RGBA `AlphaComposite+Mirror` fusion is
  wired through prepared LA/RGBA and Mirror selections in CPU batch execution,
  retaining each operation's architecture receipt. Its eligibility and
  fallback cases remain in the operation-by-mode-by-architecture parity
  matrix. Do not fold the fusion into CompositeModule.
- Public callers are the typed core functions in `ops/module_fns.rs`, Python's
  `pillow-rs-py/src/image/constructors.rs`, and JavaScript's merge/chops and
  effects wrappers. Host strings are converted to `raster::Mode` before core
  calls. The current local changes do not copy the typed-mode branch wholesale;
  exact Mode identity is carried by the Rust API and Merge pipeline descriptor.
- TODO: complete prepared-plan review against preserved mode and error paths;
  audit every direct/eager caller; and finish the global source-size review.
  Merge, Blend, and
  Composite now declare their migrated architecture routes and same-mode
  fallbacks, but their exact operation × mode × architecture parity matrix is
  pending. Add no unit tests for migrated operations; use parity cases for
  behavior, rejection, architecture selection, and fallback. No build,
  formatter, checkstyle, lint, parity, benchmark, or coverage has run.

- Chops now has a prepared CPU owner under `pool_cpu/ops/chops`, with exact
  mode folders and per-operation support IDs for all 19 ImageChops pipeline
  variants. Its mode declarations cover the native `1/L/P`, `LA/La/PA`,
  `RGB/YCbCr/HSV`, and `RGBA/CMYK/RGBa/RGBX` binary compatibility groups;
  logical operations require `1`. The typed secondary-mode contract clips
  output to the operands' overlap. Native byte kernels retain the legacy f32
  Add/Subtract ordering, LUT blends, wrapping arithmetic, and truth-based
  mode-1 logic. Constant returns L; Offset and Duplicate preserve all 25 exact
  identities. Invert reconstructs the same exact raster variant; numeric and
  internal high-depth stored-byte inversion is marked `NO_PILLOW_SUPPORT` as a
  core extension. The five internal high-depth identities also expose unary
  Constant, Offset, and Duplicate as `CoreOnly` operations with nearby
  `NO_PILLOW_SUPPORT` comments. The binary helper reuses matching raw-byte
  layouts without converting between modes. Chops Invert now declares SSE2,
  NEON, and Wasm SIMD128 strategies for its archived `1/L/P`, `LA/PA`, `RGB`,
  and `RGBA/CMYK` layouts. Those strategies and ImageOps Invert share the
  native-byte kernel in `pool_cpu/helpers/bytes/native_invert.rs`; Chops
  reconstructs the exact input mode and inverts alpha as required by its
  bytewise operation. Direct/eager caller rewiring, compatibility registration
  retirement, source review, and all verification remain pending under the
  TODO in `chops/ops.rs`. These additions are source-only and unvalidated.
- Reduce output contract decision (2026-10-29): implement typed `u16`
  averaging for `ImageI16`, `ImageI16L`, `ImageI16B`, and `ImageI16N`,
  preserving the exact source variant in the reduced `DynamicImage`. The
  archived byte-oriented 8-bit output is not the maintained contract because
  it cannot coexist with exact mode identity. The prepared Reduce family,
  support declarations, typed kernel, and caller rewiring remain pending; the
  owning TODOs are in `pool_cpu/ops/mod.rs` and
  `compute/registry/registration/geometry.rs`.
- Native-mode implementation rule (2026-10-29): do not cast, convert, or retag
  an image merely to reuse another mode's strategy. Each strategy must process
  its exact native representation and declare its output identity. Explicit
  mode conversions remain only when they are part of the operation's defined
  Pillow behavior. The high-depth Grayscale Default owners now read their
  native samples directly and write the L result without an intermediate RGB8
  raster. Put wrappers still have cross-mode conversions under an owning TODO;
  review and replace any conversion used only for kernel reuse before source
  completion.

- `raster::Mode` now names the same 25 variants as `DynamicImage`. The materialized image returns its exact enum identity, including internal high-depth representations. Alpha classification uses that identity. No separate mutable materialized mode field was added.
- Buffer-preserving mode tagging lives in `raster/dynamic/mode.rs`; Pillow spelling adaptation stays at image and binding boundaries. Materialized `LoadedData` identity comes from its exact `DynamicImage` variant, while pending pipeline output, pipeline mode cache, and mode-planner propagation use typed `Mode`. `Image::from_dynamic` wraps an already tagged raster; `Image::from_dynamic_with_mode` is fallible for explicit adapter tagging. Mode-sensitive metadata, palette, pixel access, Convert, quantize, resize, filter, transform, and backend gates use exact identities. Typed `Image::new`, `Image::frombytes`, generated-image constructors, and Draw construction take `Mode`; Python and JavaScript parse Pillow strings at their wrapper boundary. Draw bitmap/text paths use exact mode variants for supported output storage. Remaining TODOs cover mode-changing pipeline construction, prepared-executor projection, complete codec-header identity, and the remaining typed mode work in resize, filtering, merge, gradient, and pixel mutation.
- Codec translation now lives in the Pillow-owned image adapter. Existing raster codec conversion methods forward there for compatibility. Bilevel, palette, CMYK, integer, and float decode branches retain their existing byte/sample handling and produce exact raster variants. image-slash-star source is unchanged.
- ExecutionContext captures independently detected instruction capabilities once and can check precise architecture requirements, including FMA. CPU pipeline batches pass the prepared context and wrapper-resolved P/PA palette inputs to the maintained serial pool. The compatibility `Backend::Simd` request now selects that CPU pool with the detected host architecture; the separate pool, segment planner, adapter registry, and retry path are no longer active. Registry-wide admission, GPU terminal admission, and nested-operation injection remain incomplete.
- Crop now has maintained mode/default implementations for all 25 identities, an exhaustive operation-level support declaration, and composed prepared mode/architecture strategies. Its receipt retains requested backend/architecture, concrete implementation identity, and Default fallback reason. All crop strategies preserve the input identity. The five internal high-depth modes have explicit CoreOnly support and nearby `NO_PILLOW_SUPPORT` comments. Module files contain declarations/exports only.
- Crop's serial native-byte kernel preserves the original CPU row-copy loop, dimension checks, allocation accounting, and shape-error messages. Typed high-depth strategies call the existing typed raster crop helper directly. Neither helper chooses mode/backend/architecture or schedules parallel work.
- Source review compared the archived Crop adapter's exact layout and geometry guards with the maintained contracts. It admitted ordinary native bytes only for 1/L/P, LA/PA, RGB/HSV/YCbCr, and RGBA/CMYK/RGBa/RGBX/I/F; its “SIMD” path copied row spans without vector instructions. The maintained Default `ByteCrop` uses that native row-copy algorithm for every exact byte mode, including core-only modes with the same carrier. The archived I;16 path admitted only valid, nonempty I;16/I;16L/I;16B/I;16N layouts and used the shared 16-byte copy blocks plus scalar tail; these four exact modes now select that kernel under SSE2, NEON, or Wasm SIMD128 and preserve the exact output identity. Direct Crop rejects empty geometry at the checked-dimension boundary. CropBorder's archived route admitted ordinary native-byte layouts, accepted half-size empty output, and reported width errors before height errors. The maintained public wrapper and prepared contract preserve that validation order; empty outputs use typed Default, while nonempty I;16 requests may use the new native vector route. The active adapter registration is removed. Expanded operation × exact Mode × Architecture parity remains pending.
- The identical 16-byte copy block/tail kernel is shared from `pool_cpu/helpers/bytes/vector_copy.rs` by Crop, Expand, and PutData; each owner keeps its own typed request checks, row traversal, error mapping, and telemetry.
- Crop, CropBorder, grayscale, and ExtractBand now register concrete preparation callbacks returning the registry's closed `PreparedOperation` enum. Their execution consumes the already selected family plan. Remaining registry registrations still use the old callback signature and require migration with their owners; this source tree is intentionally incomplete and has not been compiled.
- `OperationInputs` borrows the exact raster and resolved palette dependencies without cloning pixels or adding a mode tag. Eager and single-operation lazy grayscale supply the resolved palette; ExtractBand supplies explicit absence because its native band copy does not consult palette entries. The maintained CPU batch receives wrapper palette metadata and supplies it only for current P/PA rasters; remaining palette-changing pipeline stages and non-batch operation callers still need operation-owned metadata integration.
- `PreparedExecution` retains the centrally resolved context. The eager CPU image callback receives its context rather than reconstructing it in the operation. The maintained CPU batch now receives that context and prepares each operation against the current mode-bearing raster. Palette dependencies, mode-changing metadata, support admission, fusion, terminals, nested inputs, and remaining eager helpers still need end-to-end injection. Separate SIMD routing and registry compatibility paths remain to be retired. No CPU completion, compilation, parity, or performance milestone is claimed.

| Preserved source/function | Maintained destination and disposition |
| --- | --- |
| `pool_cpu/legacy/ops/crop.rs::execute_crop` | Crop operation orchestration prepares one exact mode strategy; coordinate subtraction and normalized-coordinate precondition remain in the shared crop request preparation. |
| Serial row-copy branch of `execute_crop` | Shared typed `ByteCrop`, composed by each native-byte mode's Default implementation; original row order and span copying preserved. Rayon branch remains reference-only for later Parallel CPU work. |
| Typed fallback through `DynamicImage::crop_imm` | Each high-depth Default strategy calls the original typed raster `crop_buffer` helper, preserving sample order and arithmetic. |
| `pool_cpu/legacy/pool_simd/ops/adapters/crop.rs` | Source-reviewed: ordinary admitted byte layouts use the existing exact-mode Default row copy; I;16-family vector blocks and scalar tail are moved into exact CPU mode/architecture strategies with identity and telemetry preserved. CropBorder keeps width-before-height errors and empty outputs; nonempty I;16 is a native extension. Expanded operation × exact Mode × Architecture parity remains pending; no active adapter registration remains. |

### Brightness source mapping: in progress and unvalidated

- The prepared owner and all 22 implemented mode folders now sit directly
  under `pool_cpu/ops/enhance/<mode>`; operation support/orchestration is at
  `enhance/ops.rs` and common kernels are in `enhance/helpers/`. This removes
  the extra operation directory that the implementation guide forbids.
- The archived native-byte Brightness vector algorithm is composed by exact
  L, LA, RGB, RGBA, CMYK, HSV, and YCbCr mode strategies. Each mode declares
  its physical channels, active channels, and fixed-point shift shortcuts;
  the family helper receives those facts and never infers a mode. It returns
  the same typed raster, while unavailable or ineligible architectures retain
  that mode's Default strategy. Other mode strategies remain Default-only.
  CPU registry preparation points to this owner, and Backend::Simd compatibility
  requests enter CPU architecture selection; no separate adapter registration
  is active. The Brightness and Sharpness public wrappers now read mode
  acceptance and rejection errors from their `OperationId` support contracts
  instead of maintaining separate rejection lists. Source integration has not
  been compiled or parity-verified.

### Remaining enhancement source review: in progress and unvalidated

- `Image::enhance_contrast` creates a lazy operation for L, LA, RGB, RGBA,
  RGBX, CMYK, HSV, and YCbCr; other source modes enter the
  `contrast_degenerate()`/Blend composition, which can preserve mode-specific
  conversion errors. `RGBa` returns
  `ValueError("conversion from L to RGBa not supported")` during base
  construction. The prepared operation contract now
  reports that exact rejection instead of the generic wrong-mode error. The
  archived CPU leaf uses a native contrast base/LUT for finite f32 factors in
  L, LA, RGB, RGBA, RGBX, and CMYK,
  while non-finite factors and other layouts use that wrapper conversion path.
  The shared typed `contrast_base` helper now computes native base values for
  all eight queued byte identities from the exact `DynamicImage` variant.
  RGBX padding and CMYK ink remain distinct; YCbCr reads its Pillow `L` base
  from Y and uses neutral chroma for the constant base, while HSV follows its
  exact HSV-to-RGB-to-L and constant-gray-to-HSV conversions without changing
  storage mode. The calculation lives in `compute/helpers/enhance.rs`, shared
  by CPU base construction and the GPU midpoint path. Its typed
  `contrast_base_for_mode(Mode, &[u8])` entry lets per-mode strategies reuse
  the same base calculation without reconstructing an image or copying its
  pixels. The prepared Contrast owner now declares and executes typed Default
  strategies for the eight lazy modes. Finite f32 factors retain the archived
  per-channel 256-entry LUT; non-finite factors apply the same f32 blend to the
  typed native base, preserving the conversion path's alpha behavior. The
  prepared operation contract now explicitly classifies all 25 identities:
  the nine queued byte modes have native Default strategies; La and RGBa report
  their exact base-conversion rejections. One, P, PA, I, F, and all four I;16
  identities construct the existing same-mode base, then Blend rejects them
  with `image has wrong mode`. The five internal high-depth identities reject
  the L-to-source base conversion with that same error. The wrapper retains its
  conversion-first error timing; no extra Contrast strategy is warranted.
  L, LA, RGB, RGBA, and CMYK now compose their
  Default route with the archived native-byte LUT kernel under SSE2, NEON, or
  Wasm SIMD128. RGBX now uses the same four-byte LUT kernel through its exact
  SSE2, NEON, and Wasm SIMD128 strategy owners; its fourth-channel LUT uses
  the archived base value of 255, so padding samples are blended toward 255
  (and a sample already at 255 remains unchanged). The vector guard retains
  finite-f32 factor, exact native byte
  length, and the archived eight-byte minimum. Its shared mode kernel keeps
  the archived layout: one- and three-byte channels use sixteen-pixel blocks;
  two- and four-byte channels use sixteen-byte interleaved blocks. YCbCr and
  HSV remain Default-only, matching archived SIMD admission. Source review of
  all remaining identities found no additional successful Contrast route:
  One/P/PA/I/F/I;16 reach the existing Blend rejection after base construction;
  the five internal high-depth modes reject the L-to-source base conversion;
  La and RGBa retain their distinct conversion errors. Keep those exact
  rejections rather than adding strategies that change behavior. The archived
  SIMD adapter is inactive; its eligible LUT kernel now lives under exact
  mode/architecture owners. The
  public wrapper now asks the same `OperationId::Contrast` declaration as
  prepared execution whether the exact `Mode` has a queued strategy, instead
  of maintaining a second mode list. It does not compare Pillow spellings to
  choose the queued path. String conversion remains at the existing public
  conversion boundary. If a source pipeline has an explicit backend, the
  eager fallback re-locks its final blend graph to that same backend after
  materializing its existing conversion base. Degenerate enhancer setup also uses an
  exact-mode uppercase-A-band map, preserving LAB's A component; internal
  high-depth alpha cases are marked `NO_PILLOW_SUPPORT`.
- `Image::enhance_color` creates a lazy operation for L, LA, RGB, RGBA, RGBX,
  and CMYK; other source modes construct `color_degenerate()` and blend. The
  maintained owner now declares those six exact modes and dispatches to typed
  Default strategies, and explicitly classifies the other 19 modes as
  prepared-route rejections. Its native byte helper has no Pillow-string mode
  lookup; the exact mode strategy supplies the L/LA, RGB/RGBA, RGBX, or CMYK base
  layout. The archived SIMD adapter admits L, LA, RGB, RGBA, and CMYK, uses
  eight f32 lanes plus a scalar tail, and has a zero-factor RGBA grayscale
  specialization. The maintained vector strategies for L, LA, RGB, RGBA, and
  CMYK now select AVX+FMA or NEON only when the factor remains finite as f32
  and at least eight native bytes are present; otherwise they retain the
  exact-mode Default path. RGBX remains Default-only, matching the archived
  SIMD admission. The wrapper now asks the same `OperationId::ColorSaturation`
  declaration as prepared execution whether the exact `Mode` has a queued
  strategy. Eager fallback remains conversion-based for those modes and keeps
  any pinned source backend on the final blend pipeline. The archived adapter
  is inactive; its zero-factor RGBA vector path retains the archived
  integer-luma and alpha-shuffle kernel rather than routing through the generic
  eight-lane float blend.
- `Image::enhance_sharpness` rejects mode `1`, rejects P with
  `cannot filter palette images`, rejects PA with `image has wrong mode`, and
  queues the operation for other modes. The archived CPU leaf used native
  interleaved paths for L, LA, RGB, RGBA, RGBX, and CMYK; LA/RGBA preserve
  alpha, while RGBX/CMYK process all four stored bytes. Sharpness now has an
  operation-owned prepared callback, an exhaustive 25-identity support
  classifier, and exact `DynamicImage` execution arms. Each accepted mode has
  its own Default implementation and prepared plan, and all execute directly
  on their native sample type. I32/F32 read and write scalar values in their
  existing little-endian four-byte carrier. Core-only modes have nearby
  `NO_PILLOW_SUPPORT` declarations; three existing mode rejections are
  retained. The archived fused byte kernel now lives in the enhancement
  helper and is selected through exact L/LA/RGB/RGBA/RGBX/CMYK mode owners for
  SSE2, AVX, AVX2, AVX+FMA, AVX2+FMA, NEON, and Wasm SIMD128. Its guard retains
  the finite-factor, exact-layout, and minimum 3x3 requirements; border pixels
  remain a scalar tail. Source review confirms the archived SIMD admission set
  is exactly L/LA/RGB/RGBA/RGBX/CMYK and the active prepared guard preserves its
  finite-factor, exact-byte-length, and 3x3 boundary. The six architecture
  owners had a declaration-count mismatch (three entries declared for the
  seven entries returned and executed); their arrays now declare all seven.
  Source tracing is complete from public wrapper through registry preparation,
  injected execution context, exact `DynamicImage` matching, and selected
  architecture. All 22 accepted arms return their exact typed output; the
  baseline One/P/PA rejections match the active contract. The six Pillow-native
  byte modes preserve the archived Default channel, border, alpha, and RGBX
  behavior, and route to their mode-owned kernels for all seven declared
  architecture entries. The other 16 accepted modes are native extensions
  marked `NO_PILLOW_SUPPORT` in their exact-mode owners. Sharpness remains in
  the frozen CPU parity matrix after source migration is complete. The public
  wrapper queues through the prepared owner and no separate compatibility SIMD
  registration is active. No validation has run.
- Contrast registration enters the prepared owner for the nine queued byte
  modes. LAB composes its exact Convert-to-L, rounded mean, L-to-RGB Convert,
  ConvertLab, and BlendModule strategies under the injected context. La and
  RGBa expose their exact base-conversion rejections. Source review of the
  other 14 identities confirms their existing conversion-first wrapper path
  cannot complete: One, P, PA, I, F, and the four I;16 modes reach Blend's
  `image has wrong mode` rejection; the five internal high-depth modes receive
  that error while converting the constant L base back to the source mode.
  They remain explicit prepared-route rejections; do not add strategies that
  would change this public behavior. The
  archived LUT kernel comparison is
  source-reviewed for the five admitted modes; parity is pending. Sharpness's
  public wrapper queues to its prepared callback and each accepted mode has a
  Default implementation; review its operation/mode/architecture contract
  against the archive. ColorSaturation uses its prepared owner for six modes
  and has archived AVX+FMA/NEON kernels migrated into CPU architecture owners;
  eager degenerate routes for other modes and source comparison remain open.
  The independent SIMD registration has already been removed from the active
  module graph; legacy adapter sources remain inert. Do not infer acceptance from storage carrier matches: preserve
  wrapper conversions/errors and classify every exact raster identity before
  completing these owners.

### Chops source mapping: incomplete and unvalidated

- Supported native-byte modes now declare SSE2, NEON, and Wasm SIMD128
  candidates for all archived vector operations; mode `1` also declares its
  three truth-based logical vector operations. Add/Sub retain their finite
  parameter, equal-dimension, and minimum-row-width guard. Other vector
  operations require equal dimensions. Incompatible shapes or unavailable
  architectures select the same mode's serial Default implementation, which
  retains overlap clipping. All kernels read and write the selected mode's
  native bytes and reconstruct its exact mode without conversion.
- The archived Multiply/Screen widening and divide-by-255 arithmetic,
  min/max/difference/modulo byte lanes, Overlay/HardLight divide-by-127 LUT
  formulas, SoftLight integer formula, and mode-`1` canonical truth results
  have been moved into the family-owned vector helper. This is source
  migration only: explicit SIMD-request routing, compatibility SIMD
  registration retirement, source review, compilation, parity, and benchmark
  evidence remain pending under `pillow-rs#3ptb`.
- The archived Multiply→Screen scalar and vector formulas now have a prepared
  native-mode fusion for L/LA/RGB/RGBA. CPU batch execution admits it only for
  adjacent operations sharing the same secondary `Image`, matching mode and
  dimensions, and matching Multiply/Screen architecture selections. It keeps
  the intermediate multiply truncation and performs one output traversal.
  Source integration is unverified; explicit SIMD-request routing, SIMD
  compatibility retirement, source review, compilation, parity, and benchmark
  evidence remain pending under `pillow-rs#3ptb`.
- The public Rust Chops adapter checks `raster::Mode` from known wrapper
  identity, materializing the source only when that typed identity is unknown;
  this removes its last Pillow-string mode comparison. Internal high-depth
  unary support is classified as CoreOnly at the operation descriptor.

| Preserved source/function | Maintained destination and disposition |
| --- | --- |
| `pool_cpu/legacy/ops/chops.rs::op_chops_add` through `op_chops_subtract_modulo` | `chops/helpers/binary.rs` preserves native-byte formulas and row clipping; exact mode support and secondary mode compatibility live in `chops/ops.rs` and each mode's `ops.rs`. |
| `pool_cpu/legacy/ops/chops.rs::op_chops_logical_and` through `op_chops_logical_xor` | `chops/helpers/binary.rs` uses truth semantics on raw mode-1 bytes and emits canonical 0/255 bytes. |
| `pool_cpu/legacy/mod.rs::fused_multiply_screen` and `pool_cpu/legacy/pool_simd/ops/adapters/chops/kernels/fused_multiply_screen.rs` | `chops/helpers/fusion.rs` preserves serial and vector arithmetic for exact L/LA/RGB/RGBA. `chops/ops.rs` prepares both mode/architecture declarations; `batch_execution.rs` routes adjacent operations with the same operand through the prepared fusion. Explicit SIMD-request routing and verification remain pending. |
| `pool_cpu/legacy/ops/chops.rs::op_chops_constant`, `op_chops_offset`, `op_chops_duplicate` | `chops/helpers/unary.rs` keeps L output for Constant and same-mode native behavior for Offset and Duplicate. |
| `pool_cpu/legacy/ops/chops.rs::op_chops_invert` | `chops/helpers/unary.rs` inverts stored bytes and `chops/helpers/output.rs` reconstructs the exact requested raster identity; high-depth extensions are separately declared CoreOnly. The archived byte-vector kernel is now shared from `pool_cpu/helpers/bytes/native_invert.rs` by the exact-mode Chops and ImageOps architecture strategies. |

### Grayscale source progress: incomplete and unvalidated

- The family entry point classifies every exact raster identity. It declares the existing `ImageLaPremultiplied8` (`La`) rejection and prepares concrete strategies for the other 24 identities. Every successful result is `ImageLuma8`. This is source implementation progress, not a verified Pillow support claim.
- Default mode implementations retain the archived source handling: nonzero bilevel samples become white; LA copies its luma; RGB/RGBA/RGBX use the existing rounded fixed-point loop; RGBa first applies the existing integer unpremultiplication; CMYK keeps the fused ink subtraction; HSV keeps its lookup-table RGB conversion; YCbCr keeps packed-word Y extraction and the original fallback; I/F preserve their raw little-endian scalar clipping. LAB retains the archived byte-carrier route pending the full parity gate rather than introducing a replacement algorithm during migration. `I;16*` inputs apply the archived per-sample clip directly from native `u16` storage. Internal LA16/RGB16/RGBA16/RGB32F/RGBA32F inputs convert each native sample directly into the luma output kernel without creating an intermediate RGB8 image; the conversion currently preserves the original channel scaling and alpha-drop behavior.
- Palette strategies require explicitly resolved wrapper metadata. P retains its existing palette expansion or absent-palette index path; PA retains expansion through an empty palette when no colors are attached. Palette alpha and per-pixel alpha keep their existing separate roles. A single lazy P/PA Grayscale op now reaches its prepared strategy before wrapper expansion; broader wrapper-owned operation dispatch still requires rewiring.
- RGB AVX2 and NEON, YCbCr SSSE3 and NEON, CMYK NEON, and LA/RGBA/RGBX/RGBa NEON implementations have concrete mode/architecture owners. RGB AVX2 independently declares SSE2, SSSE3, and AVX2. Existing vector arithmetic, bounded blocks, scalar/padded tails, and vector counters were retained. CMYK receives a prepared NEON token rather than constructing a SIMD level per block.
- The archived interleaved RGB/LA/four-byte vector traversal, channel gathering, and packing are shared once by named NEON, x86, and Wasm SIMD128 strategies. The non-NEON CMYK math and serial block/tail traversal are preserved in a shared family helper. The original portable channel gather serves YCbCr on 32-bit x86 and Wasm; the archived x86_64 scalar gather fallback is not claimed as a vector specialization. LA names its actual SSE2/SSSE3 byte path rather than claiming AVX2 arithmetic. The four I;16 identities share the same original clipped-RGB helper. Scalar I/F and CMYK loops were moved to typed Default owners and existing color facades delegate to those loops, avoiding a second maintained implementation. HSV and RGBa gained typed buffer entry points for their existing conversion helpers.
- Operation support now distinguishes fixed versus preserved output identity, precise instruction requirements, native-layout guards, semantic source-length errors, and unavailable build/hardware/request fallbacks. Contextual assessment reports ready, rejected, or deferred. Unknown source length, unresolved palette metadata, or an unknown CropBorder request cannot be reported as an executable selection. Crop and CropBorder use the same assessment type and prepared-selection carrier.
- The execution context supplies a centralized permitted-implementation relation while checking instruction capabilities independently, including SSE2 and statically enabled Wasm SIMD128. Operation strategies do not probe hardware. Selection retains the first preferred declaration's failure reason when a narrower specialization or Default is chosen; later candidates do not overwrite it. Backend/context injection outside the new families remains incomplete.
- The internal LA16/RGB16/RGBA16/RGB32F/RGBA32F grayscale routes are explicitly CoreOnly and carry nearby `NO_PILLOW_SUPPORT` comments; they consume native samples and produce L without an intermediate mode raster.
- Grayscale registry preparation and eager CPU callback wiring are present. Tiny-RGB eager ExtractBand uses the prepared CPU strategy for CPU Default and the compatibility SIMD request; both now pass the selected architecture through the eager receipt. Single-op lazy P/PA grayscale passes the original mode-bearing raster and resolved `PaletteInput` into its prepared strategy before wrapper-wide expansion. The shared telemetry frame retains vector and scalar-tail counters, selected architecture for prepared strategies, and mode-conversion accounting by exact `Mode` identity. Remaining direct eager operations still need source review and context reconciliation under the `compute/eager.rs` TODO. Serial pool/batch integration, broader palette dependencies and transitions, and final source review across all target configurations remain pending. Named x86/Wasm routes and capability metadata have not been compiled or executed. Grayscale is not marked complete. Existing string-mode compatibility callers and scalar helper placement remain part of the full migration review.

| Preserved source/function | Maintained destination and disposition |
| --- | --- |
| `pool_cpu/legacy/ops/grayscale.rs::op_grayscale` | `grayscale/ops.rs` composes explicit mode strategies and preserves the La rejection; mode Default owners contain the original scalar behavior. |
| `native_ycbcr_luma` and `native_ycbcr_luma_packed_blocks` | `grayscale/ycbcr8/default/grayscale_impl.rs`; original packed byte extraction, dimension eligibility, fallback, and allocation accounting retained. |
| `color/grayscale.rs::pil_grayscale_inner` native branches | Typed Default owners use the existing shared scalar luma loop; the old facade remains for unmigrated callers and is not a substitute for completing their mode/context migration. |
| `color/conversion.rs::i_to_l` and `f_to_l` scalar loops | `grayscale/i32/default/grayscale_impl.rs` and `grayscale/f32/default/grayscale_impl.rs`; original color facades delegate to the typed loops. |
| `color/grayscale.rs::cmyk_to_grayscale` loop | `grayscale/cmyk8/default/grayscale_impl.rs`; the exact source-length rejection is checked during preparation from the same contract used by assessment. |
| `color/conversion.rs::hsv_to_rgb` and `ops/pil_resize/alpha.rs::unpremultiply_alpha` RGB branch | Shared typed buffer helpers preserve existing HSV factors and integer alpha restoration; grayscale's HSV/RGBa strategies inject their exact source buffers. |
| Wrapper palette expansion before color-dependent operations | P/PA Default preparation composes the existing expansion helpers with typed luma conversion; single-op lazy grayscale uses it directly, while remaining color-dependent callers are pending. |
| Archived SIMD `grayscale/kernels/rgb_avx2.rs` | `grayscale/rgb8/avx2/grayscale_impl.rs`; masks, arithmetic, short-input behavior, and bounded vector tail retained behind the prepared feature contract. |
| Archived SIMD RGB NEON functions in `channels.rs` | `grayscale/rgb8/neon/grayscale_impl.rs`; original specialized RGB loop and padded tail retained. |
| Archived SIMD `ycbcr_ssse3.rs` and NEON extraction in `ycbcr.rs` | Exact `ycbcr8/ssse3` and `ycbcr8/neon` implementation files; leaf feature probes removed, load bounds retained. |
| Archived SIMD NEON math and serial vector traversal in `cmyk.rs` | `grayscale/cmyk8/neon/grayscale_impl.rs`; explicit prepared token, unchanged arithmetic; Rayon scheduling remains archive-only for later Parallel CPU work. |
| Archived SIMD interleaved two-/three-/four-channel loops in `channels.rs`, channel masks, and pack helper | `grayscale/helpers/interleaved.rs::{simd_pack_u16x16,grayscale_block,grayscale_interleaved}` and the shared CPU `helpers/channel/gather.rs::gather_interleaved_channel`; concrete grayscale strategies preserve their original loops. The lane gather is shared with put's native NEON extraction. RGB NEON keeps its dedicated optimized loop. |
| Non-NEON `cmyk_grayscale_block` and serial traversal in archived `grayscale/kernels/cmyk.rs` | `grayscale/helpers/cmyk.rs::{block,execute}`; typed CMYK SSE2/SSSE3/AVX2/Wasm implementations compose the original arithmetic, zero-initialized output, padded tail, and saturating counters. Parallel scheduling remains archived. |
| Portable branch of `legacy/ops/imageops/colorize.rs::gather_native_channel` reached by archived YCbCr | Shared CPU `helpers/channel/wide.rs::gather`, called by YCbCr's Wasm and 32-bit x86 owners and put's matching native byte implementations. x86_64 retains its dedicated SSSE3 implementation or serial Default. |
| 32-bit x86 RGB generic-wide branch of archived `native_grayscale_bytes` | `rgb8/avx2/grayscale_wide_impl.rs` plus RGB SSE2/SSSE3 owners retain the generic loop. x86_64 AVX2 continues using its archived intrinsics, rather than replacing them with that loop. |

The following is source-level declaration bookkeeping, not executed architecture
coverage. Inspection used the pinned local `wide` 1.6.1 implementation:
`i8x16::swizzle_relaxed`, `u16x16` storage/widening/arithmetic, and
`u8x16::narrow_i16x8`. Whole-binary target requirements still apply.

| Grayscale strategy | Declared build and instruction contract |
| --- | --- |
| RGB x86_64 AVX2 | Compiled on x86_64 with a target-feature entry point; independently requires SSE2, SSSE3, and AVX2 before preparation selects it. |
| RGB 32-bit x86 AVX2; RGBA/RGBX/RGBa/CMYK wide AVX2 | The original wide loop is compiled with static AVX2; requires SSE2 and AVX2, plus SSSE3 when the build enables its shuffle branch. |
| RGB/RGBA/RGBX/RGBa/CMYK wide SSSE3 | Static SSSE3 without static AVX2; requires SSE2 and SSSE3. |
| RGB/RGBA/RGBX/RGBa/CMYK wide SSE2 | Static SSE2 without static SSSE3 or AVX2; retains the archived scalar shuffle with vector arithmetic/packing rather than inventing an intrinsic kernel. |
| LA wide byte gather | Static SSSE3 selects the SSSE3 declaration; otherwise static SSE2 selects SSE2. No AVX2 arithmetic or AVX2-named LA implementation is declared. |
| YCbCr x86 | x86_64 uses the existing target-feature SSSE3 kernel. 32-bit x86 uses its archived portable gather in the static SSE2/SSSE3 build. The x86_64 non-SSSE3 path remains serial Default. |
| RGB/LA/RGBA/RGBX/RGBa/CMYK/YCbCr Wasm | Requires a wasm32 build with static SIMD128; the injected context records that build capability without a leaf probe. |
| Existing NEON strategies | Require the AArch64 NEON build and captured NEON capability; CMYK gets its token once during preparation. |
| No implemented vector target | Uses the exact mode's serial Default. The old generic array-emulation source remains in the archive; it is not registered as a fictitious architecture. |

### ExtractBand source progress: incomplete and unvalidated

- `put/ops.rs` exposes `EXTRACT_BAND_SUPPORT` and concrete prepared strategies for all 25 raster identities. Each result is `ImageLuma8`. This describes the existing internal PipelineOp, whose defensive index behavior remains available to direct internal callers. The public `Image::getchannel` wrapper still owns selector validation, the I/F rejection, and the P-mode identity copy with retained palette. Its remaining string-based metadata boundary is migration debt, not a new typed-boundary completion claim.
- Mode declarations expose the original index rules: luma replicates into the first three bands and returns opaque alpha for higher indices; RGB selects its first three bands and returns opaque alpha for higher indices; LA/PA and four-byte carriers clamp to their final native band. The four I;16 identities now clip their native u16 samples directly to L bytes, preserving the original 0..255 clipping and opaque-alpha fallback without constructing an intermediate RGBA image. Internal LA16/RGB16/RGBA16/RGB32F/RGBA32F strategies read their native sample types directly and use the existing numeric-to-byte conversion; internal LA16 selects its actual luma or alpha band. The previous high-depth RGBA conversion path was removed from ExtractBand.
- Serial defaults preserve logical-raster truncation for trailing storage, four-pixel LA/RGBA unrolling, scalar tails, empty-image handling, allocation order/counters, and existing shape errors. Typed forwarding is inlined; the actual loops remain in their implementation/shared helper owners.
- Existing vector byte movement is composed for LA, PA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, and HSV. x86_64 SSSE3 requires independently captured SSE2 and SSSE3 and uses the original bounded intrinsic gather. 32-bit x86 retains the original static SSSE3 or SSE2 wide gather. Wasm requires static SIMD128. NEON retains its original two-/three-channel gathers and four-channel structure load, with its token prepared once rather than probing in a leaf. Byte-only gathers do not gain an AVX2 label.
- The declaration's `ExtractBandBytes` guard uses typed `RequestFacts::ExtractBand { index }` alongside dimensions/source length. Empty or overlong storage and out-of-native-range indices select the same mode's Default. Missing index/content facts remain deferred when needed; known-ineligible layouts can select Default without materialization. Mode-only metadata never invents a parameter value.
- Shared gathers moved to CPU `helpers/channel`: grayscale and put reuse actual lane-selection/wide algorithms. The old private color gather adapters were removed after inspecting active callers. Their existing boundary test moved with the shared SSSE3 kernel; its inputs/assertions remain unchanged and its call now establishes the raw helper's safety contract explicitly. The two archived CPU ExtractBand regressions were restored under put with an injected Default context; their shapes, selectors, and assertions remain unchanged. Archived SIMD adapter tests still require relocation with the final API review.
- CPU eager ExtractBand now reaches the typed prepared registry callback. Tiny-RGB `Image::getchannel` and loaded-image `GetData` use that same prepared CPU architecture strategy for compatibility SIMD requests; the exact mode-bearing source chooses the kernel. GetData writes into its caller-owned final byte slice without allocating an intermediate L image, and its receipt records the selected CPU architecture. `PreparedExtractBand::execute_into` itself returns `None` when the selected exact mode has no caller-owned sink strategy and executes only for the nine native byte identities that write directly to the final slice; no parallel mode-support list remains beside it. The remaining pipeline SIMD/GPU string admission table has an actionable TODO at the wrapper boundary. The public wrapper's two Cargo-parallel-feature exclusions were removed so enabling that feature does not change serial CPU eager admission. The pool facade/batch executor, shared metadata transitions, registry-wide assessment APIs, and the other put operations remain incomplete. The route is source-only and unvalidated; this is not a completed put-family or CPU checkpoint.

| Preserved source/function | Maintained destination and disposition |
| --- | --- |
| `legacy/ops/color.rs::op_extract_band` luma and RGB branches | Exact mode Default owners compose `put/helpers/extract_band.rs::{luma,rgb}` with original logical-raster collection and opaque-alpha behavior. |
| Native LA/RGBA branches of `op_extract_band` | Mode Default owners call the original four-pixel algorithm through the compile-time-layout `grouped` helper; matching raw carriers retain their native bytes. |
| High-depth branch of `op_extract_band` | Four I;16 Default owners use `clipped_luma16`; internal LA16/RGB16/RGBA16/RGB32F/RGBA32F owners use `native` to extract and convert only the selected native sample. No intermediate RGBA raster is created. |
| SSSE3 functions formerly in `color/grayscale.rs` | Shared CPU `helpers/channel/ssse3.rs`; original allocation, masks, bounds, stores, initialization, and counters remain. Production callers receive eligibility from prepared strategies rather than probing hardware in an adapter. |
| Archived `native_channel.rs` NEON into-buffer functions and structure load | Shared CPU `helpers/channel/neon.rs`, composed by the concrete put mode/NEON owners; unused allocating facades were not recreated. |
| `legacy/pool_simd/ops/adapters/extract_band.rs` caller-owned GetData output | The nine byte-mode ExtractBand declarations select a prepared CPU Default or architecture strategy; `put/helpers/extract_band_vector.rs` writes vector results directly into the final slice and reports vector/tail counts without an intermediate L allocation. |
| Existing direct SSSE3 and archived CPU ExtractBand tests | `helpers/channel/tests.rs` and `put/tests/extract_band.rs`; source relocation/signature adaptation only, not executed evidence. |

The structure guide remains the authority for exact maintained paths. This ledger records function ownership and remaining work, not a competing path grammar. No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage command has run for these changes.

### PutPixel source progress: incomplete and unvalidated

- `put/ops.rs` exposes separate `PUT_PIXEL_SUPPORT`, `PUT_PIXEL_BATCH_SUPPORT`, and `PUT_PIXEL_VALUE_AT_SUPPORT` descriptors. The first two classify all 25 identities: the 16 byte-backed modes retain native writes and exact output identity, while the nine high-depth identities retain `NotImplementedError("putpixel not supported for this image type")`. The scalar value-at contract classifies all 25 identities, accepts I/F and all four I;16 variants with exact output identity, and rejects other modes. The public I/F and I;16 wrappers select this CPU Default path and record its receipt.
- Each supported mode has typed serial single-write and ordered batch implementations. The existing batch algorithm clones once, checks each operation and coordinate in source order, and preserves last-write-wins behavior. Default preparation does not scan the batch before that original loop.
- The existing masked byte store is composed by 15 modes' SSE2, NEON, and wasm SIMD128 owners. Premultiplied La remains Default-only, matching the archived vector adapter's admission. LAB retains its PutPixel-specific vector admission rather than inheriting drawing support. No AVX/SSSE3-named store was invented for this SSE2-width bitwise implementation.
- RGBA alone declares the existing vector batch strategy. Its original requirements remain explicit: at least two writes, one complete vector, exact raster length, only in-bounds PutPixel descriptors, and no palette-index tags. Preparation inspects these facts only when a declared architecture is available under the captured build/hardware/policy; metadata queries can defer unknown run facts. The plan borrows the inspected immutable operations and executes its retained concrete choice without a retry or reselection.
- Source review against pinned `wide` 1.6.1 confirmed `u8x16` storage and bitwise operations use static SSE2, AArch64 NEON, or wasm SIMD128 branches. Other targets retain serial Default. The vector helper preserves global byte phase, padded short stores, the original checked offset errors, logical-raster truncation, and vector counters. Shared typed helpers contain no mode inference, hardware probes, or parallel scheduling.
- The PutPixel registry callback constructs the family-owned prepared plan, and the serial pool routes consecutive PutPixel runs through its prepared batch strategy. The CPU batch executor now checks a contiguous RGB PutPixel prefix followed by Thumbnail before materializing the writes. The typed fusion is restricted to the CPU backend, reduces natively by 2x2, updates only affected blocks, and binds sparse active-row facts to the exact prepared RGB Resize or ResizeBoxed strategy. It accounts one output boundary and records each fused operation. This source integration is unverified; its parity/resource regression review and public wrapper transitions remain pending.
- Two archived vector regressions were relocated to `put/tests/put_pixel.rs`. Vector-edge shapes, write values/order, byte comparison, and unsafe-run rejection assertions remain. The former string override `Some("RGB")` is represented by an exact RGB variant with the same bytes and dimensions, since a contradictory mode override no longer belongs in compute. The archived pool resource/telemetry regression has not run against the maintained executor. No test has run.

| Preserved source/function | Maintained destination and disposition |
| --- | --- |
| `legacy/ops/put.rs::op_put_pixel` | Typed mode `default/put_pixel_impl.rs` owners compose `put/helpers/put_pixel.rs::write`; unchanged native sample placement, one clone, and exact mode identity. |
| `Image::putpixel_mode_scalar` for I/F | `put/ops.rs::PUT_PIXEL_VALUE_AT_SUPPORT` classifies the exact mode and binds a CPU Default selection. `put/i32/default/put_pixel_impl.rs` and `put/f32/default/put_pixel_impl.rs` mutate the exact native four-byte sample in place through the shared checked byte-range helper; the wrapper records the selected strategy. |
| `Image::putpixel_l16` for I;16 variants | The same operation contract selects exact `i16`, `i16l`, `i16b`, or `i16n` Default owners. They write the existing native u16 sample in place without converting modes; the wrapper retains modulo-65536 normalization and records the selection. Source-only; unverified. |
| `legacy/ops/put.rs::{apply_put_pixel_batch,op_put_pixel_batch}` | The same Default owners compose the ordered generic `write_batch` helper with concrete pixel constructors; original descriptor/bounds errors and last-write-wins order remain. |
| Archived vector `native_putpixel_layout` and shape/image admission | Linked mode and architecture declarations in `put/ops.rs`, mode `ops.rs`, and `put/helpers/put_pixel_strategy.rs`; typed source identity replaces string/layout guesses. |
| Archived `simd_put_pixel_native` | `put/helpers/put_pixel_vector.rs::single`, reached through exact mode/architecture implementation owners; original masked store and finalization preserved. |
| Archived nonblending `draw_line_vector_block` branch | `put/helpers/masked_store.rs`; compile-time native channel width removes the unused blending decision from PutPixel without changing source-byte phase, masks, or arithmetic. Drawing migration must reconcile reuse with its blending implementation later. |
| Archived `simd_rgba_putpixel_batch_supported` | `put/helpers/put_pixel.rs::rgba_run_facts` plus `StrategyGuard::RgbaPixelWriteRun`; the same mode declaration drives assessment and preparation. |
| Archived `simd_rgba_putpixel_batch` and `kernels/rgba_batch.rs::apply_rgba_batch` | Typed RGBA architecture owners compose `put/helpers/put_pixel_vector.rs::{batch,apply_rgba_batch}`; immutable admitted writes retain order, one frame clone, and vector-store telemetry. |
| Archived PutPixel inline vector tests | Two source regressions relocated to `put/tests/put_pixel.rs`; pool-level telemetry/resource behavior still needs review against the maintained executor. |

Remaining put-family work includes typed wrapper boundaries and complete caller reconciliation. PutData and both alpha operations have source owners, but remain unvalidated and are not end-to-end complete. The structure guide remains the placement authority. No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage has run. The latest commit is `3f110aa47` on `develop`; current PutPixelValueAt changes are unverified and the active CPU refactor remains incomplete.

### PutData source progress: incomplete and unvalidated

- Source review confirmed that PutData's payload layout can differ from the source image in direct core calls. The archived CPU operation converts to the requested byte layout in that case; GPU admission explicitly retains this CPU fallback. The refactor therefore retains a typed payload parameter independently of the source variant rather than removing it as a redundant mode hint.
- `put/ops.rs::PUT_DATA_SUPPORT` now classifies every source identity through the same declaration that binds its concrete prepared plan. All 25 sources have serial implementations. Their payload contract enumerates the 14 canonical byte-mode identities represented by the existing normalization interface, including RGBA's RGBa/RGBX aliases. The 11 other typed payload identities have an explicit rejection; this does not broaden the public wrapper's accepted strings or replace its I;16 mutation path.
- Each mode's Default implementation receives its exact buffer type and a prepared payload strategy. It retains the original nonempty full-payload replacement paths for matching native layouts; 16 formerly clone-only index/color-space/raw-four-byte writes now use the native prefix helper. Cross-layout typed conversions, I;16 clipping, prefix truncation, untouched suffix, zero-size behavior, and `putdata: buffer error` failure remain. No source-mode or channel inference occurs inside the conversion or copy loops.
- Output relations are explicit in the mode contract: a matching native byte layout preserves the exact source identity; a cross-layout or high-depth conversion returns the corresponding L/LA/RGB/RGBA carrier. The same payload declaration generates accepted identities, compatible native widths, payload strategy construction, and the inspectable output table.
- All 16 byte-backed source identities have native-copy SSE2, NEON, and wasm SIMD128 owners. This retains the archived adapter's logical-mode fallback, including its La/LAB physical-copy admission through an explicitly supplied compatible payload. The nine high-depth sources use their serial strategy. Exact source length and compatible payload identity guard specialization; a missing/ineligible architecture selects that same source mode's Default strategy. The original u32 byte-count error, full-payload single copy, partial source/prefix copies, padded vector tail, counters, and empty finalization remain in `put/helpers/put_data_vector.rs`.
- Registry PutData registration now returns its family-owned prepared plan. Its legacy `op_put_data` facade import was removed. `PipelineOp::PutData` and its request facts carry exact `Mode`; CPU contracts and GPU consumers select behavior from that identity. GPU maps supported modes to existing shader layout codes, while `PixelMode` remains for separate alpha-operation contracts.
- Typed native byte finalization is now shared through `image_utils::typed_bytes_to_buffer`. The existing raw-byte facade delegates to this same length/truncation/shape implementation, and PutPixel no longer duplicates it. Concrete operation strategies construct the exact result variants. Original raw-byte error strings and checked nonempty/empty policies are retained.

| Preserved source/function | Maintained destination and disposition |
| --- | --- |
| `legacy/ops/put.rs::op_put_data` complete-payload native branches | Exact source mode `default/put_data_impl.rs` owners compose `put/helpers/put_data.rs::native`; nonempty replacement and fallback construction order retained. |
| `Image::putdata_value_at` immediate host-callback mutation | `image/data_access/put.rs` retains normalization and callback ordering; after loading, the exact planned `Mode` is checked against `DynamicImage::mode()` and the raw byte carrier is mutated through `as_bytes_mut`. Its immediate write selects the exact-mode `PutDataValueAt` CPU Default contract after prior pipeline work is materialized and records the actual selection without delaying the write. Wrapper metadata and conversion review remain pending. |
| `Image::putdata_l16_bytes` raw-byte fast path | Current implementation still builds a zero-filled native-order byte plane in the wrapper and writes through generic luma16 access. The adjacent `pillow-rs#3ptb` TODO requires mode-owned PutData strategies while retaining row mapping, byte order, clipping, and error order. |
| `Image::putpixel_mode_scalar` numeric I/F immediate write | `put/ops.rs::PUT_PIXEL_VALUE_AT_SUPPORT` classifies all 25 identities and links I/F to CPU Default mode owners. The wrapper normalizes the same four native bytes, selects from `DynamicImage::mode()`, mutates in place through the shared raw-byte helper, and records its selection. Source-only; unverified. |
| `Image::putdata_l16_bytes` raw byte fast path | Source TODO: route the row-shaped raw-byte mapping through exact-mode PutData strategies, preserving byte order, per-row zero-fill, clipping, and current error order. |
| `op_put_data` partial and cross-layout branches | Same-carrier writes use `put/helpers/put_data.rs::native`; true cross-layout branches compose `converted`/`Prefix::apply` with concrete ConvertBuffer functions. No DynamicImage reclassification occurs in the loop. |
| Existing I;16-to-byte raster conversions reached by `op_put_data` | Four endian-identity owners compose the original clipped sample traversal through `put/helpers/put_data.rs::clipped`. |
| Archived vector `native_put_data_layout` and shape/image admission | Mode-owned `PUT_DATA_SUPPORT`, `ByteDataPayload`, and `PutDataNative` declarations bind source identity, explicit compatible payloads, exact native storage, and prepared architecture selection. |
| Archived `simd_put_data` and `native_bytes.rs::copy_native_bytes` | `put/helpers/put_data_vector.rs`, composed by exact source/architecture owners; original allocation and native byte traversal retained. Reconcile broader native-copy reuse as other families migrate. |
| `image_utils.rs::raw_bytes_to_image_with_empty` shape finalization and PutPixel's interim duplicate | One typed buffer finalizer in the existing canonical utility, reused by both raw facade and typed PutPixel/PutData owners. |

### PutAlpha and PutAlphaData source progress: incomplete and unvalidated

- Each operation now has an exhaustive 25-mode entry-point declaration, explicit outputs, typed source buffers, and a concrete Default owner. Constant alpha keeps native L/LA/P/PA layouts, promotes RGB/CMYK/other byte carriers as before, clips I;16 samples through the existing helper, and retains the CMYK MULDIV255 conversion path.
- The original architecture loops are placed in mode-owned folders: constant alpha has vector owners for L, P, RGB, RGBA, RGBX, and CMYK; image-backed alpha has L/LA/P/PA/RGB/RGBX/CMYK owners. The same declaration binds build and architecture requirements, native-byte guards, minimum vector groups, and same-mode Default fallback. RGBA mask replacement remains in Default because its archived “SIMD” path only cloned and wrote alpha bytes; it does not claim a vector implementation.
- Secondary masks retain their exact 25-mode identity in a borrowed strategy selected before pixel processing. The Default contract converts masks using the original typed-to-L behavior and keeps zip/prefix output semantics. Architecture owners require the archived native L-mask shape and exact source storage. The large RGB-mask row path, complete vector groups, scalar tails, and original receipt counters are retained.
- CMYK alpha calls the shared serial RGB conversion helper, so CPU execution does not enter the optional Rayon route. The shared u16-to-byte vector packing primitive is now owned by the CPU channel helper and reused by grayscale and alpha kernels.
- Registry callbacks now prepare opaque operation plans and no longer import the old alpha operation facades. Existing alpha kernel assertions were relocated with their original inputs and expectations; they were not run.
- The maintained serial CPU pool and prepared batch dispatcher now receive routed context and execute registered operation plans in order. The Pillow wrapper passes resolved palette metadata through prepared CPU batches, including automatic SIMD-to-CPU recovery; each operation receives it only while its current raster identity is P or PA. Consecutive PutPixel runs use the prepared PutPixelBatch mode/architecture declaration, retain serial Default, account one output boundary, and report the fusion count. Consecutive transpose runs compose into one prepared mode/architecture strategy; cancellable runs preserve the packed-storage guard and defer the owned identity copy to the batch terminal, which records the allocation and boundary. Consecutive L/LA/RGB/RGBA point operations compose native byte LUTs and enter the prepared Eval strategy; identity LUT chains retain a single owned copy. Multiply+Screen now uses its prepared native Chops fusion in CPU batches. AlphaComposite+Mirror and Draw runs now use prepared CPU strategies; the RGB PutPixel+Thumbnail fusion is active with a narrower mode contract, while its remaining mode/fallback and terminal caller review stays open, as do canonical pipeline/binding mode boundaries, metadata transitions, registry-wide assessment, remaining operation families, and full source/caller review. No compiler, formatter, checkstyle, lint, test, parity, benchmark, or coverage has run; this source work is incomplete and unvalidated.

### Transpose-family source progress: incomplete and unvalidated

- `transpose/ops.rs` declares exhaustive operation-level support for Flip, Mirror, and Transpose with typed mode dispatch, prepared plans, registry callbacks, and receipts. Every exact raster identity has a Default owner and preserves its mode tag. Pipeline registration no longer passes a Pillow mode string into these operations.
- The byte-backed mode owners use checked native row/pixel copying and fall back to the same generic typed image transforms for malformed or nonstandard storage. High-depth owners preserve typed sample values. Palette indices, premultiplied samples, color-space bytes, numeric carriers, and byte-order mode identities are moved without conversion.
- Transpose covers all seven methods with exact-mode Default owners and typed method preparation. Its native byte path retains checked dimensions, same-dimension row copying, serial/tiled axis swaps, the original 32-pixel tile threshold, and the RGBA 16-pixel tile size. Typed modes use the original mode-preserving pixel-coordinate transforms.
- An AVX2 RGBA mode strategy now consumes the injected architecture plus exact native layout, dimensions, and axis-swap method guards. Its `rgba8/avx2` kernel ports the archived AVX2 tile transform and handles incomplete edges through byte copies. This path has not been compiled or parity-checked.
- The archived wide-lane Mirror kernel is shared in `transpose/helpers/native_mirror.rs`; `One`, `ImageLuma8`, and `ImagePalette8`; `ImageLumaA8` and `ImagePaletteAlpha8`; `ImageRgb8`, `ImageHsv8`, and `ImageYCbCr8`; and `ImageRgba8`, `ImageCmyk8`, `ImageRgbPremultiplied8`, `ImageRgbx8`, `ImageI32`, and `ImageF32` bind it through exact-mode declarations for SSE2, NEON, and Wasm SIMD128. Each prepared strategy restores its own `DynamicImage` variant. RGBA uses the same helper in its custom AVX2-transpose strategy owner. Native row length and channel-group guards select these implementations, with the same-mode Default mirror as fallback. The `ImageI16`, `ImageI16L`, `ImageI16B`, and `ImageI16N` Flip/Mirror native-copy routes now use `transpose/helpers/native_luma16.rs`, preserving typed samples and exact output identities under SSE2, NEON, and Wasm SIMD128 strategies with same-mode Default fallback. The packed-word RGBA Rotate270 kernel now runs in the RGBA Default owner behind the original x86_64, 32-pixel tile threshold, and four-pixel divisibility guards; it has no ISA requirement and remains a Default implementation detail. The archived portable `wide` transpose tile, gathered-row, and scalar-edge paths are migrated to `transpose/helpers/native_transpose.rs` and selected by exact-mode SSE2, NEON, and Wasm SIMD128 strategies, with same-mode Default fallback. Its conditional Rayon row collection is excluded from CPU and remains for the later `pool_parallel` stage. The duplicate SIMD byte-Flip compatibility route must be retired after CPU routing reaches the prepared owner. Registry/executor and other callers remain part of the broader incomplete migration. No compiler, formatter, lint, test, parity, benchmark, or coverage command has run.

| Preserved source/function | Maintained destination and disposition |
| --- | --- |
| `legacy/ops/imageops/pixel.rs::{flip_native_rows,op_flip}` | Exact-mode Default implementations under `transpose/<mode>/default/flip_impl.rs`, sharing `transpose/helpers/pixel.rs::flip_vertical_bytes`; generic typed fallback preserves the old fallback behavior. |
| `legacy/ops/imageops/pixel.rs::{mirror_byte_row,mirror_byte_rows,op_mirror}` | Exact-mode Default implementations under `transpose/<mode>/default/mirror_impl.rs`, sharing checked native byte reversal; typed fallback matches the original generic horizontal transform. |
| `legacy/ops/transpose.rs::{execute_transpose,transpose_bytes_serial,transpose_bytes_tiled,transpose_bytes_rows}` | `transpose/helpers/pixel.rs` plus each mode's `default/transpose_impl.rs`; serial byte layout and tiling behavior are retained without Rayon. |
| `PipelineOp::{Transpose,Flip,Mirror}` registry callbacks | `transpose/ops.rs` prepares the exact mode strategy with injected `ExecutionContext`; registration no longer supplies a string mode. |
| `legacy/ops/transpose/kernels/transpose_rgba/x86_64.rs` | Migrated to `transpose/rgba8/default/rotate270_tiled.rs`; it runs only under the original x86_64, tile-size, and divisibility guards and remains part of the Default strategy because it has no ISA requirement. |
| `legacy/pool_simd/ops/adapters/transpose/kernels/rgba_avx2.rs` | Ported to `transpose/rgba8/avx2/transpose_impl.rs`; declared through the `TransposeRgbaAvx2` guard. Source is unvalidated. |
| `legacy/pool_simd/ops/adapters/flip.rs::simd_flip` byte route | It delegates to `img.flipv()` and has no distinct vector kernel; retire the duplicate compatibility route after prepared CPU routing is active. Its I;16 native row-copy route is in `transpose/helpers/native_luma16.rs`. |
| `legacy/pool_simd/ops/adapters/flip.rs::{simd_flip,simd_mirror}` I;16 native-copy paths | `transpose/helpers/native_luma16.rs`, selected by the exact `ImageI16`, `ImageI16L`, `ImageI16B`, and `ImageI16N` Flip/Mirror strategy declarations. Native byte length must be exact and nonempty; each mode's same-mode Default implementation remains the fallback. |
| `legacy/pool_simd/ops/adapters/flip.rs::simd_mirror` byte-backed modes | `transpose/helpers/native_mirror.rs`, selected by exact per-mode declarations for `One`, `ImageLuma8`, `ImagePalette8`, `ImageLumaA8`, `ImagePaletteAlpha8`, `ImageRgb8`, `ImageHsv8`, `ImageYCbCr8`, `ImageRgba8`, `ImageCmyk8`, `ImageRgbPremultiplied8`, `ImageRgbx8`, `ImageI32`, and `ImageF32`; RGBA uses its custom strategy owner. |
| `legacy/pool_simd/ops/adapters/transpose/kernels/portable.rs::{transpose_gathered_row,transpose_native_tile,transpose_native_row_group,transpose_native_tiled}` | Portable wide-lane tile, gathered-row, and scalar-edge paths are in `transpose/helpers/native_transpose.rs`, reached through exact-mode SSE2, NEON, and Wasm SIMD128 owners with same-mode Default fallback. The legacy conditional Rayon row scheduling and collection helpers remain archived for the later `pool_parallel` stage. |
| `legacy/pool_simd/ops/adapters/transpose.rs::rgba_avx2` and `kernels/rgba_avx2.rs` | Ported to `transpose/rgba8/avx2/transpose_impl.rs`; declared through the `TransposeRgbaAvx2` guard. Source is unvalidated. |

### Transform-family source progress: incomplete and unvalidated

- `transform/ops.rs` declares all 25 exact input identities at the family entry point. Twenty Pillow modes have native Default strategies; the five internal high-depth identities retain explicit `NotImplementedError` rejection. Every maintained byte result preserves its exact input variant. Default affine-nearest owners retain the archived specialized L, LA, and three-byte RGB-layout kernels; RGB, HSV, and YCbCr share only the same-layout `Rgb` sampler while preserving their exact wrappers. The I32/F32 owners retain their raw four-byte carriers and their typed scalar filtered paths. Four I;16 identities have distinct mode owners using typed u16 buffers and explicit native/little/big byte order; filtered I;16 behavior preserves Pillow's byte-plane filter semantics without creating or routing through another image mode.
- Public Transform entry points in `ops/transform.rs` create `PipelineOp::Transform`, and `registry/registration/transform.rs` prepares the mode-aware CPU owner. `transform_public` obtains the cached exact `Mode` through `mode_identity()` and passes it into fill normalization and palette/One filter selection; fill arity, scalar, carrier, and named-color resolution match enum variants. The Python adapter also uses the exact `mode_identity()` for P-specific fill parsing instead of branching on `Image.mode()` text. The typed `color::getcolor_for_mode` preserves the existing Pillow string resolver's result for all 20 externally named identities without spelling round-trips and preserves carrier-equivalent named-color results for the five internal identities; operation support still rejects those five modes. Sampling, F sample-word and omitted-fill, P forced-nearest, PA bicubic-filter, LA/RGBA/RGBX identity, I;16 byte-order, and typed color resolver behavior belong in the mode-by-architecture parity matrix. `ops/transform/fill.rs` handles P fill identity/color normalization and its type/value errors. Architecture evidence must cover declared SSE2/NEON selection where compiled, unavailable-architecture fallback, and Default when a mode has no specialization. The remaining legacy suite stays as reference; no supported active caller needs its `explicit_mode` override.
- Mode owners inject `ExecutionContext` into their prepared strategy. Architecture selection occurs from each mode's declaration. All 14 native-byte modes admitted by the archive (`1`, L, P, LA, PA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, HSV, I, and F) declare AVX, AVX2, SSE2, NEON, and Wasm SIMD128 routes for affine and perspective/quad nearest through `TransformByteKernels`. L, LA, RGB, HSV, YCbCr, RGBA, and CMYK also declare affine bilinear through the same exact-mode support contract. Their wrappers use the shared interpolation helper; only LA and RGBA select alpha-aware premultiply/unpremultiply behavior. Each mode uses its exact typed wrapper and retains its output identity. LAB and La retain their same-mode Default strategy because neither is admitted by the archived native-byte layout contract.
- The four I;16 identities declare AVX, AVX2, SSE2, NEON, and Wasm SIMD128 affine nearest/bilinear routes with a typed u16 output wrapper and an exact native two-byte guard. Nearest sampling stays in u16. Pillow's archived filtered I;16 contract processes the first `width` bytes of each native row and restores the second fill byte; the typed mode owners preserve that byte-plane arithmetic in a scratch buffer while input and output remain exact I;16 variants, with no `DynamicImage` mode conversion.
- The archived adapter's admitted byte routes are affine nearest for the 14 native byte identities `1`, L, P, LA, PA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, HSV, I, and F; affine bilinear for L, LA, RGB, HSV, YCbCr, RGBA, and CMYK; projective/quad nearest and mesh nearest for those 14 byte identities; and affine nearest/bilinear for all four I;16 identities. Projective/quad bilinear kernels exist in the archive but are not admitted by its support predicate and remain out of scope. No active mode strategy may restore the old image-mode inference or carrier retagging.
- The archived mesh entry passes admission for nearest native-byte images, but its loop evaluates each sample scalarly and only groups eight pixels for bookkeeping; it has no vector kernel. Keep mesh in each mode's Default strategy rather than advertising it as an architecture specialization.
- Source review compared Python TransformData classification and public method/error routing with the pre-migration wrapper. Its malformed-input and method-specific error branches remain unchanged; mode selection and fill normalization now use exact `Mode`. Palette fill, P forced-nearest, and PA filtered behavior remain parity cases. Mesh remains Default because the archived implementation only groups scalar samples and has no vector kernel. No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage command has run for this family.

### Draw-family source progress: mode and architecture strategies migrated; review pending

- `pool_cpu/ops/draw/ops.rs` classifies all 25 exact `raster::Mode` identities
  at the operation entry point. The 20 Pillow modes have a same-mode Default
  strategy; the five internal high-depth modes are rejected with the existing
  `unsupported image mode` value error and carry `NO_PILLOW_SUPPORT` comments.
  The family uses `Mode` from the input `DynamicImage`; it does not add a
  wrapper mode or infer a Pillow spelling.
- The archived scalar scan-conversion helpers now live under
  `pool_cpu/ops/draw/helpers`. Exact mode owners write directly to their typed
  byte or I;16 buffers; One retains its expanded-byte carrier and archived
  direct ink-byte writes, and the four
  I;16 owners retain the legacy packed-ink byte-order distinction. The shared
  writer is compile-time specialized by channel count and does not convert the
  source raster through RGBA.
- All ten Draw pipeline variants use the prepared Draw callback. Contiguous
  Draw operations select one mode strategy from the injected CPU context and
  clone the source canvas once. The GPU draw-control preview calls the same
  native Default owner using the exact source identity; GPU preflight now
  supplies that explicit CPU Default context to the Draw operation instead of
  having the pool_cpu family construct one internally. GPU exact-host execution
  and automatic CPU preflight fallback also pass an explicit CPU Default context
  through `execute_batch_with_context`, rather than entering the context-free
  pool method.
- The archived native-byte span kernels now live under `draw/helpers/simd` and
  are reached through exact mode/architecture owners for the 16 byte-backed
  Pillow identities. The operation declaration binds SSE2, NEON, and Wasm
  SIMD128 availability to the `DrawNativeSpans` admission guard; unsupported
  requests retain the same mode's Default owner. Four I;16 identities retain
  their typed Default paths, and the five internal high-depth identities are
  explicitly rejected with nearby `NO_PILLOW_SUPPORT` comments. Kernel output
  reconstruction uses the exact source `Mode` and does not infer it from a
  Pillow spelling or channel count.
- Direct Draw Bitmap and text composition are eager operations outside the
  pipeline `PreparedDraw` type. Bitmap's native loops live under each exact mode's
  `draw/<mode>/default/bitmap_impl.rs`, with shared byte-layout, clipping,
  mask, and I;16 helpers under `draw/helpers`. `Draw::bitmap` now selects the
  exact raster-mode Draw strategy through an injected `ExecutionContext` and
  records the selected architecture and strategy fallback reason. Direct
  loops are ineligible for the geometry span kernels and therefore use
  same-mode Default; compatible
  non-CPU bitmap requests still delegate to Paste's prepared strategy. The
  `DrawRequestKind` distinguishes geometry, Bitmap, and Text strategy requests.
  The explicit RGBA-on-RGB context remains separate from the raster identity.
  `SUPPORT` declares Bitmap's accepted mask identities and validates its context
  relation against exact raster facts; `pool_cpu/ops/draw/ops.rs` now matches
  the selected implementation's exact raster mode and calls its mode owner.
  Eager Draw reads the current typed `Image::mode_identity()` so pending
  pipeline outputs are respected; `orig_mode` remains for original-mode
  restoration and the explicit RGBA-on-RGB context. The public wrapper only
  materializes the input and restores P palette metadata on the direct Default
  route. The RGB8 fast path and CMYK/L-mask row path remain specialized.
  Direct Text resolves the same operation support declaration using exact
  raster/context modes and embedded-color eligibility, then dispatches from the
  selected raster mode. Eager Text telemetry records selected architecture and
  fallback. The context selection and fallback behavior are source-only.
  Bitmap and text now route all 20 Pillow modes through exact mode matches.
  Bitmap preserves native One/L/LA/P buffers; directly handles La/PA, CMYK,
  RGBa/RGBX, YCbCr/LAB/HSV, I/F, and each I;16 identity; and rejects the five
  internal high-depth modes before destination materialization. Text's native
  loops now live in exact mode Default owners for One/L/LA/La/PA, RGB/RGBA/
  RGBa/RGBX, CMYK, YCbCr/LAB/HSV, I/F, and each I;16 identity. Shared text
  geometry, gray-alpha composition, three-band arithmetic, numeric carriers,
  and I;16 normalization live under `draw/helpers`; mode owners retain exact
  variant matching and reconstruction. The explicit RGBA drawing context over
  an RGB destination still selects the RGB owner when its native RGB carrier is
  available and retains the existing fallback behavior. Text rejects the five
  internal high-depth modes before destination materialization. Its support
  facts now carry raster/context compatibility, P palette presence,
  embedded-color eligibility, FreeType mask representation, text-ink
  provenance, and packed foreground ink while retaining the exact raster
  identity in `DynamicImage`. The support contract defers P Text/Bitmap
  admission when palette metadata is unknown, and image palette presence is
  queried without cloning its entries. Text execution now dispatches from the
  prepared selection in `pool_cpu/ops/draw/ops.rs` into exact-mode owners; the
  public Draw wrapper prepares the font mask and restores P palette metadata
  around the returned raster. Direct Text selection is composed from `SUPPORT`;
  prepared coverage/RGBA pixels travel with the selected input to avoid a
  borrowed-mask/moved-input boundary. Remaining source review compares each
  mode's ink resolution, alpha/premultiplication behavior, palette semantics,
  I/F raw carrier rules, and error ordering against archived CPU and wrapper
  contracts, then reconciles direct admission with retained architecture cases.
  No unit tests are planned for this migration; correctness evidence is parity
  across operation × exact Mode × available Architecture, including selection,
  fallback, and rejection receipts. No build, formatter, checkstyle, lint,
  parity, benchmark, or coverage command has run for this family; source review
  remains in progress.

### ImageOps source progress: incomplete and unvalidated

- Public ImageOps mode admission in `ops/imageops/point.rs` now matches typed `Mode` identities rather than Pillow strings. Exact mode lookup uses the wrapper's cached identity; when a lazy pipeline has no identity yet, the current materialization fallback is explicitly pending removal once all pipeline transitions preserve a known exact mode.
- `imageops/ops.rs` owns an exhaustive classifier for all 25 exact modes. The byte modes have per-mode strategy owners and preserve their exact mode identity. Numeric I/F and internal high-depth carriers retain their existing shape-error behavior for bytewise Invert, Posterize, and Solarize. Invert's LA, PA, RGBA, and CMYK routes are explicitly CoreOnly because the Pillow wrapper rejects those four identities; each owner has a nearby `NO_PILLOW_SUPPORT` comment.
- CropBorder declares CoreOnly support for the five internal high-depth identities with nearby `NO_PILLOW_SUPPORT` comments and preserves each exact raster mode. Colorize declares Pillow support only for exact `ImageLuma8`; its other 24 mode identities are explicitly rejected with the same empty `AssertionError` as the public wrapper. This removes the legacy non-L-to-L projection from the prepared path and keeps Colorize native to its actual Pillow input mode.
- Each mode's `default/invert_impl.rs` performs the same `255 - byte` transform as `legacy/ops/imageops/pixel.rs::op_invert`; the scalar byte loop is shared in `imageops/helpers/pixel.rs`. Strategy preparation receives `ExecutionContext`, and registry preparation now returns the closed `PreparedOperation` plan.
- Named Expand and Pad colors now stay on the typed `Mode` path through `color::getcolor_for_mode`; they no longer format a Pillow spelling only to resolve the same mode again. The shared resolver marks its internal high-depth color results `NO_PILLOW_SUPPORT`; operation support still decides whether a core-only route can execute.
- Source review compared the archived native-byte adapter's exact One/L/P, LA/PA, RGB, and RGBA/CMYK admissions and its SSE2/NEON/Wasm SIMD128 wide-kernel path. Maintained mode owners retain native-byte inversion and same-mode output. The Pillow wrapper rejects LA/PA/RGBA/CMYK, so their explicit `NO_PILLOW_SUPPORT` routes are CoreOnly; LA/PA/RGBA invert every stored byte to match their Default semantics, while the archived ImageOps adapter preserved alpha. CMYK inverts all four bytes in both paths. The separate SIMD registration is no longer active.
- The public Invert wrapper and point-run fusion callers have been source-reviewed; fusion composes LUTs through prepared Eval under the same exact mode and injected context. The archived owned-buffer in-place Invert path is restored for Invert and InvertChops: exact-mode support and injected context select the architecture before the owned intermediate is mutated in place; caller-owned input remains untouched. Operation × exact Mode × Architecture parity remains pending, including CoreOnly routes through target-side parity cases.
- Posterize and Solarize now declare all 25 modes at `imageops/ops.rs`, with prepared strategies for L, RGB, and core-only P. P applies each transform directly to index bytes, preserves its palette, and is marked `NO_PILLOW_SUPPORT` because Pillow's ImageOps `_lut` rejects that mode; other unsupported identities retain Pillow's mode-specific `OsError`. Default kernels are mode-owned; L, RGB, and P declare SSE2, NEON, and Wasm SIMD128 strategies using the exact-mode `NativePointwiseBytes` guard and shared vector transform. Prepared execution matches the selected architecture. Public wrappers queue typed operations; custom/high-threshold Solarize uses single-band Eval, whose palette-aware pipeline keeps P indexes and metadata attached. Point-run fusion composes through prepared Eval under the exact mode and injected context. Operation × exact Mode × Architecture parity remains pending, with P handled as a target-only core extension.
- High-threshold and custom-comparison Solarize requests deliberately use `eval_replicated` and enqueue Eval rather than `PipelineOp::Solarize`; the CPU registry prepares them with the mode-aware Eval owner. The remaining SIMD compatibility route is tracked with Eval below.
- `Autocontrast` and `Equalize`/`EqualizeMasked` now have operation-owned support declarations and prepared CPU callbacks. L and RGB use their exact Default kernels and guarded SSE2, NEON, or Wasm SIMD128 kernels. P is explicitly rejected by Autocontrast with the existing `NotImplementedError`; Equalize expands P through the injected palette to RGB and selects a declared RGB data kernel under the P strategy. Source comparison against the archived CPU and SIMD routes covers histogram admission, cutoff/equalization arithmetic, LUT application, masks, empty images, and direct callers; masks accept exact `One`/L identities and preserve mode-before-size error ordering. CropBorder also routes through its prepared imageops callback; its archived ordinary-byte admission and empty/error ordering are represented. Operation × exact Mode × Architecture parity remains pending.
- `Expand` now has an operation-owned support declaration for all 25 exact source identities. Each mode has a Default owner under its own folder, performs border fill and row copying in its native sample representation, and returns the same exact `DynamicImage` variant. P/PA preserve their index samples; typed I;16 and internal high-depth modes never route through an 8-bit mode. The five internal high-depth modes are core-only and carry nearby `NO_PILLOW_SUPPORT` comments; fill bytes become same-valued native numeric samples. `ExpandedDimensions` exposes source layout, border facts, and checked output allocation through the same contract used during preparation. Registry preparation now enters the closed CPU `PreparedExpand` plan instead of the archived direct callback.
- The new `geometry/ops.rs` owner declares typed support for Contain, Cover, Fit, and Scale across all 25 exact identities. It preserves the archived aspect-ratio rounding, zero-size error order, Fit bleed/centering box, and P nearest-filter rule, then composes the request with a prepared exact-mode Resize plan using the same injected context. Its support descriptors name Resize or ResizeBoxed as child contracts, and the prepared operation exposes each child's selected architecture alongside the outer geometry selection. Pad now routes through its prepared exact-mode owner described below. The five internal modes carry nearby `NO_PILLOW_SUPPORT` declarations. Geometry parity and archived SIMD route comparisons are not complete.
- Pad now has an operation-owned prepared entry at `pool_cpu/ops/geometry/pad/ops.rs`, with exhaustive support for all 25 `Mode` identities and a typed `PadParameters` request contract. Each mode has its own `ops.rs` plus serial `default/pad_impl.rs`; implementations keep the exact output variant and copy that carrier's native samples through the Pad-local row helper. P forces Nearest in its composed Resize child. The same support declaration composes Resize, and the prepared Pad receipt exposes the child's concrete selection. The five internal high-depth identities include `NO_PILLOW_SUPPORT` comments and use their native carriers. The archived SIMD row fill/copy path now lives in `helpers/native_vector.rs`; 13 exact byte modes declare SSE2, NEON, and Wasm SIMD128 strategies guarded by exact native byte layout, while other modes use same-mode Default. Luma8's filtered SSE2/NEON/Wasm SIMD128 Resize strategies can write directly into Pad's full-width canvas window; serial L/RGBX retain their Default window strategies. The TODO in `geometry/pad/ops.rs` tracks moving the remaining archived SIMD full-width convolution modes into exact mode/architecture Resize owners and retiring the old SIMD compatibility route after caller reconciliation. The Pad default and row fill/copy kernels were source-reviewed against `legacy/ops/imageops/pad.rs`, `legacy/ops/imageops/pad/native.rs`, and `legacy/pool_simd/ops/adapters/pad.rs`; the remaining SIMD convolution routes and callers still need review. This source has not been compiled, parity-checked, or benchmarked.
- The archived 128-bit fill/copy implementation now lives in `imageops/helpers/expand_vector.rs` and is shared by exact-mode byte strategies. The same declaration selects SSE2, NEON, or Wasm SIMD128 when built and available. Its `ExpandVector` guard enables full border fills for one- and two-byte layouts; three- and four-byte layouts use it for border-zero identity copies, matching the archived vector work. Other requests select the same-mode Default implementation. The vector kernel constructs the original exact variant rather than inferring output mode from channel count.
- The archived CPU Expand cases and SIMD byte-fill/copy regressions now live in `imageops/tests/expand.rs`; mode-string overrides were replaced with exact `DynamicImage` variants, and the old contradictory carrier/mode checks are no longer constructible. These tests have not been run.
- Expand source review is complete against the archived CPU and SIMD routes. The public wrapper queues into the exact-mode registry; the architecture guard selects vector fill for one-/two-byte layouts and border-zero copies for three-/four-byte layouts, matching the kernel's actual branches. Exact-mode output identity and all declared modes are retained. Operation × exact Mode × Architecture parity remains pending; no active adapter registration remains.
- The ImageOps implementation and support matrix have not been compiled, parity-checked, or benchmarked. No source-completion claim is made.

| Preserved source/function | Maintained destination and disposition |
| --- | --- |
| `legacy/ops/imageops/pixel.rs::op_invert` | `imageops/<exact-mode>/default/invert_impl.rs`, with bytewise arithmetic in `imageops/helpers/pixel.rs`; successful output preserves the exact `DynamicImage` variant. |
| `legacy/ops/imageops/pixel.rs::{op_posterize,op_solarize}` | `imageops/luma8/default/{posterize_impl,solarize_impl}.rs` and `imageops/rgb8/default/{posterize_impl,solarize_impl}.rs`; exact-mode prepared plans own admission and serial execution. Archived vector kernels are folded into each mode's SSE2/NEON/Wasm SIMD128 wrappers via `imageops/helpers/native_bytes.rs`. Compare the archived SIMD admission behavior with the exact-mode owner; no independent registration remains active. |
| `legacy/ops/imageops/colorize.rs::{colorize_lut,op_colorize}` | Pillow's exact-L LUT construction and execution live in `imageops/helpers/colorize.rs` and the L mode owner; the archived vector kernel is composed by native-L SSE2, NEON, and Wasm SIMD128 strategies. Exact nonempty native-L bytes guard architecture selection, and the 65,536-pixel path split, padded tails, and vector telemetry are retained. The public wrapper validates L before queueing Colorize. Non-L projection through a temporary L raster was removed from the prepared CPU route. |
| `legacy/ops/imageops/{histogram.rs,pixel.rs}::{autocontrast_lut,equalize_lut,op_autocontrast,op_equalize_with_mask}` | `imageops/helpers/histogram.rs` retains histogram arithmetic; `autocontrast.rs` and `equalize.rs` declare mode support, mask admission, and prepared selection. L/RGB Default kernels live under their mode folders. Equalize P expands the injected palette to RGB and declares the resulting architecture route. |
| `legacy/ops/imageops/expand.rs::op_expand` and `legacy/pool_simd/ops/adapters/expand.rs::simd_expand` | `imageops/expand.rs` declares all 25 source-mode routes; every mode binds its contract to `default/expand_impl.rs`, and the 14 Pillow byte modes compose the shared `imageops/helpers/expand_vector.rs` kernel for guarded SSE2, NEON, and Wasm SIMD128 execution. The exact mode owner restores its own output variant. Source admission and the pipeline caller are reviewed; operation × exact Mode × Architecture parity remains pending. The separate registration is removed. |
| `legacy/ops/imageops/geometry.rs::{op_contain,op_cover,op_fit,op_scale}` and `legacy/ops/imageops/pad.rs::op_pad` | `geometry/ops.rs` owns Contain/Cover/Fit/Scale and dispatches Pad to `geometry/pad/ops.rs`. Pad declares all 25 exact-mode routes, composes a prepared Resize selection, preserves native carrier samples, and routes through the injected CPU context. The archived byte vector fill/copy path now runs through 13 exact mode/architecture owners. Serial L/RGBX direct-to-canvas writes use their prepared Default Resize strategy. TODO in `geometry/ops.rs`: compare archived SIMD full-width convolution behavior and direct callers with the prepared CPU strategies; add any missing mode/architecture case. The separate SIMD caller is no longer active. No completion or validation is claimed. |
| Archived Expand CPU and SIMD regressions | Migrated to `imageops/tests/expand.rs` with the original native fill bytes, output dimensions, byte comparisons, and architecture-request comparisons; exact mode tags replace string overrides. Not executed. |
| `legacy/pool_simd/ops/adapters/histogram.rs` | Histogram LUT application now runs through the L/RGB architecture-owned wrappers and shared `imageops/helpers/histogram.rs` vector kernel. Source admission and direct pipeline callers are reviewed; operation × exact Mode × Architecture parity remains pending. The separate registration is removed. |
| `legacy/pool_simd/ops/adapters/effects.rs::{native_invert,native_invert_copy,native_invert_in_place}` | `imageops/<mode>/{sse2,neon,wasm_simd128}/invert_impl.rs` composes the shared native-byte kernel under exact mode and architecture support. The batch executor restores the owned-buffer in-place route for Invert and InvertChops through `pool_cpu/helpers/bytes/native_invert.rs`; caller-owned inputs are not mutated. Operation × exact Mode × Architecture parity remains pending. |
| `imageops/ops.rs` mode classifier and prepared entry points | Exhaustive exact-mode classification and operation-owned prepared callbacks for ImageOps operations; public wrappers and batch fusion route through the registry or prepared Eval owner. Remaining support limitations and parity scope are recorded with each operation. |

### Color3DLut source progress: CPU source review complete; parity pending

- `color3dlut/ops.rs` now exposes an exhaustive source-mode contract: RGB, RGBA, and CMYK have Pillow-defined strategies; YCbCr, LAB, and HSV have native `CoreOnly` strategies marked `NO_PILLOW_SUPPORT`; the remaining 19 identities retain the existing `image has wrong mode` rejection. The declaration requires an explicit RGB/RGBA/CMYK target and enforces each target's channel capacity. Its output rule is the exact typed target mode. The three core-only source modes apply the table directly to their native three-byte samples without converting or retagging the source.
- The original signed 10.6 table preparation, 18.15 coordinate scaling, trilinear interpolation order, RGBA/CMYK fourth-byte behavior, and RGB output projection are in `color3dlut/helpers/lut.rs`; mode-owned Default files bind each source identity to native RGB or four-byte sample helpers. RGB, RGBA, and CMYK no longer pass through `DynamicImage::get_pixel`'s RGBA view or allocate an intermediate RGBA image. CMYK output uses the exact `ImageCmyk8` variant, and a three-channel table preserves the source fourth byte for RGBA and CMYK.
- The prepared CPU registry callback enters the exact mode strategy. RGB, RGBA, and CMYK each declare Default and the available architecture strategy. The archived eight-lane fixed-point interpolation kernel is shared by those mode owners; its dimensions, channel count, exact table length, native source byte length, and eight-pixel threshold are part of architecture selection, with Default fallback for ineligible requests. YCbCr, LAB, and HSV declare their own Default and available architecture owners with the same native three-byte kernel; their Pillow-external support is explicitly `CoreOnly` and excluded from Pillow parity. The typed Rust `Image::color3dlut` entry now admits these same three core-only sources when the caller supplies a supported target mode; Python and JavaScript continue to parse external mode strings before calling it. Source review found the public image method builds a `PipelineOp::Color3DLut` and reaches `prepare_pipeline` through the CPU registry; there is no separate active direct CPU callback. Comparison with the archived SIMD adapter confirms its source modes, eight-pixel eligibility, fixed-point interpolation, target-channel behavior, and output mode are represented by the prepared CPU strategies. The separate SIMD adapter is no longer active. Expanded CPU operation × source mode × target mode × architecture parity remains pending, including Default fallback, invalid LUT requests, vector tails, and core-only strategy receipts. GPU source-mode snapshot encoding must consume the prepared mode transition in the later GPU pass.
- RGBX now declares architecture strategies for every target in its existing target contract. Target L reuses RGBX's AVX2, SSSE3, SSE2, NEON, and Wasm SIMD128 grayscale kernels; target RGB reuses the exact four-byte-to-RGB SSSE3/NEON kernels; and the SSE2/NEON wrappers select direct native-buffer strategies for LA, RGBA, CMYK, YCbCr, HSV, I, and F. These routes ignore the padding byte for color/scalar outputs, set it to 255 when producing RGBA/LA, and retain the exact output `Mode`. RGB and RGBX share the fixed-point opaque-LA helper. This source migration has not been compiled or validated.
- CMYK8 now declares the archived CMYK→RGB specialization for SSSE3, SSE2, and NEON. The selected kernels read the native four-channel CMYK carrier and emit RGB directly using Pillow's `muldiv255` arithmetic; other supported CMYK targets stay on the same-mode Default implementation. The SSSE3 route gathers interleaved channels with the existing channel helper, and the SSE2/partial-block route uses the existing portable wide-channel helper. This source migration has not been compiled or validated.
- The four I;16 source identities now consume their native `u16` samples directly. Their Default implementations and SSE2, NEON, and Wasm SIMD128 wrappers preserve the archived eight-sample packing behavior for L, LA, RGB, RGBA, CMYK, I, and F targets. All four I;16 identities preserve native unsigned samples for I/F. YCbCr and HSV remain Default routes. These architecture routes are source-reviewed but have not been compiled or validated.
- No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage command has run for this source.

### Eval source progress: prepared CPU mode strategies source-reviewed; parity pending

- `eval/ops.rs` now exposes an exhaustive contract for the 25 exact identities. `One`, `L`, `LA`, `La`, `P`, `PA`, `RGB`, `RGBA`, `RGBa`, `RGBX`, `CMYK`, `YCbCr`, `LAB`, and `HSV` have native byte-LUT Default strategies with one table per native band. `I`, `F`, all four Pillow I;16 identities, and the five internal high-depth identities retain explicit rejection because `PipelineOp::Eval` accepts byte LUTs. Direct scalar point transforms now have a separate `PointScalar` support declaration for I/F/I;16 and per-mode Default implementations under `eval/<mode>/default`; their public entry points inject CPU/Default context, and the algorithms retain the original native sample arithmetic.
- Each admitted mode's contract includes an exact `256 * native_channels` LUT length and exact native source-byte dimensions. Its Default implementation preserves the input variant and applies the archived affine-LUT or per-band lookup loop directly to that mode's bytes. P/PA index bytes remain native; the image wrapper continues to own palette metadata. The per-mode Default modules now follow the explicit `mode/default/eval_impl.rs` layout.
- The nine identities admitted by the archived SIMD LUT adapter (`One`, `L`, `P`, `LA`, `RGB`, `HSV`, `YCbCr`, `RGBA`, and `CMYK`) declare SSE2, NEON, and Wasm SIMD128 strategies beside their exact mode contracts. A shared family helper owns the reusable channel LUT kernel; RGB's little-endian AArch64 TBL4 and affine paths now live at `eval/rgb8/neon/eval_impl.rs` and are selected directly by the RGB mode owner. Other modes use the shared portable-wide implementation for their declared architecture, while identity LUTs retain the native-copy path. The kernel helpers contain no Rayon scheduling.
- `PointFloat` now declares exact `One`, `L`, and `P` strategies with a typed 256-entry f64-table contract, exact native-byte shape, and fixed `ImageF32` output. Per-mode Default leaves live under `eval/<mode>/default/eval_float_impl.rs`; the shared native gray-byte to F32 kernel lives under `eval/helpers/point_float.rs`. The public wrapper injects CPU/Default context; each leaf reads its native byte samples directly without converting the source mode.
- Source review compared the legacy operation, native-LUT adapter, batch routing, current public point wrappers, and registry preparation. The archived SIMD admission set is exactly `One`, `L`, `P`, `LA`, `RGB`, `HSV`, `YCbCr`, `RGBA`, and `CMYK`; its channel counts, channel-major LUT layout, exact byte lengths, 16-sample table lookup blocks, padded row tails, and RGB little-endian AArch64 TBL4/affine paths are represented by the corresponding prepared CPU mode/architecture owners. Empty images and unsupported architectures fall back to each exact mode's Default strategy. The old separate registration and mode-string inference are no longer active.
- The source comparison found and corrected a request-facts crossover in the `One`, `L`, and `P` owners: byte Eval now supplies `EvalLut` facts, while PointFloat supplies `PointFloatLut` facts. Direct scalar point callsites, exact-mode validation order, mode-preserving byte outputs, and fixed `ImageF32` float-LUT outputs are accounted for. The archived Rayon row scheduler remains outside maintained CPU execution and is reserved for the later `pool_parallel` pass.
- Expanded parity remains pending across operation × exact Mode × Architecture, including all supported byte modes, vector tail widths, empty inputs, identity LUTs, affine RGB NEON, scalar point modes, and rejected modes. No unit tests or validation commands have run for this source.
- No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage command has run for this source.

### ConvertLab source progress: incomplete and unvalidated

- `color/ops.rs` declares an exhaustive support contract over all 25 identities. `ImageRgb8` selects the native RGB8 Default strategy and produces exact `ImageLab8`; every other identity retains an explicit `ValueError("image has wrong mode")` outcome. This prepared RGB8-to-LAB strategy is Pillow-scoped. The pinned Pillow 12.2 wrapper uses ImageCms for RGB-family inputs and normalizes other supported sources to RGBA first.
- The public `Image.convert(Mode::ImageLab8)` adapter in `ops/convert/dispatch.rs` normalizes its input before it queues `PipelineOp::ConvertLab` (including the existing I;16 to L to RGB path). Therefore the prepared operation consumes exact RGB8; input mode normalization stays at the explicit conversion boundary. Its RGB byte-length contract permits an empty image only when the native byte buffer is also empty. Commit `5e4b509d4` adds the composed source-to-LAB route to Convert's top-level contract: Pillow identities report Pillow scope, the five internal high-depth identities report CoreOnly, and La retains the exact `conversion from La to L not supported` rejection. The prepared Convert plan captures source→RGB (or I;16→L→RGB) and ConvertLab selections before execution; the wrapper gates its lazy graph with the same source/target declaration.
- The serial Default implementation lives at `color/rgb8/default/convert_lab_impl.rs`. It retains the archived LittleCMS CLUT interpolation, tie-breaking, rounding, and byte packing order, and constructs `DynamicImage::ImageLab8`. Shared CLUT/profile data remains in `lab.rs`; no architecture-specific ConvertLab kernel exists. The old standalone SIMD registration was scalar control only and has been removed from registration.
- Registry support and prepared-operation wiring now identify ConvertLab through this owner. General Convert remains in progress as detailed below; its public Rust boundary is typed, but the archived SIMD source/target reconciliation and broader caller review are pending. Correctness evidence will come from the operation × exact Mode × Architecture parity matrix; this migration does not add unit tests.
- No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage command has run for this source.

### Convert source progress: Default source strategies in progress; incomplete and unvalidated

- Archived alpha route review: see the exact route and empty-input findings in
  “Current source-only continuation.” The broader adapter/kernel map below is
  still pending for ordinary layout and color-space routes, typed byte-source kernels, generic dispatcher guards, build availability, and scalar-control reconciliation
  paths.

- The public Rust `Image::convert` API now takes exact `Mode` for its destination and reads the source via `mode_identity()`. Its dispatch and the conversion helpers use `Mode` matches, including nonstandard color/scalar conversion, binary output, palette output, I;16 routing, and matrix targets. `color::convert_from_nonstandard` now accepts `Mode`. Python and JavaScript parse valid Pillow strings into `Mode` before delegation; an invalid string is retained only as an external compatibility error input so matrix-validation order and diagnostics remain unchanged. Enhance, paste source preparation, and module Composite now pass typed modes to conversion. This removes the previously listed string conversion gap, but does not complete the archived source/target matrix review or prepared strategy caller audit.
- After existing prestandard handling, each Convert source/target pair declared by `ModeSupport` now enters the prepared plan; the same declaration decides eligibility. Source review found `assess_convert_target` rejected typed targets `One` and `P` before checking their declared target lists, which blocked the eager binary and prepared-palette wrappers even after they materialized their source. Removed that stale unconditional rejection so the exact declared target membership now controls admission; this correction is source-only and unverified. Ordinary One conversions use the native One strategy instead of widening through L. Conversion to target One now dispatches through an exact source-mode owner selected from the injected CPU Architecture context; those mode entry points compose the shared Default threshold and Floyd–Steinberg helpers. SIMD requests fall back to Default until a binary architecture kernel is declared. The archived RGB/LAB error-diffusion distinction is preserved. One matrix conversion retains the archived L normalization before entering the prepared matrix strategy. The public matrix path remains eager, but `ConvertMatrix` now has a top-level contract, exact source-mode selection, injected CPU architecture context, and a shared Default implementation that applies the preserved RGB sample projection and matrix arithmetic directly to each mode's native samples without a temporary RGB image. No archived SIMD matrix kernel exists, so SIMD requests use the declared Default fallback; GPU matrix execution remains a TODO beside the wrapper. The matrix strategy admits L/RGB targets for all 25 exact source identities; non-RGB source projections are explicitly CoreOnly. Declared I;16 source pairs for I/F, HSV, and YCbCr now use native u16 strategies; all four I;16-to-P/PA paths now clip native u16 samples directly into identity-palette indices without an intermediate L image. The four I;16 sources now declare cross-identity I;16 targets and execute from their native u16 buffers in their exact mode Default strategies; same-identity conversion preserves samples, while cross-identity conversion retains the archived clipped-L widening result. These target pairs have no architecture kernel and explicitly fall back to Default. LA16, RGB16, RGBA16, RGB32F, and RGBA32F declare the four I;16 targets as core-only routes and project native samples directly into the same luma bytes as their existing Default grayscale strategies, without an intermediate L raster. SIMD requests for these Default-only targets select the mode's Default implementation. LA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, HSV, PA, LAB, I, and F now declare all four I;16 outputs and execute through their exact mode Default strategy; their native pixels are converted directly to the prior L-byte result where needed, without a temporary L image. I32 preserves its target-specific u16 clamp for I;16/I;16L/I;16B and clipped-L behavior for I;16N; F32 retains its clipped byte conversion. One and L keep their existing lazy prepared path. The eager recursive wrapper bridge was removed. `P` retains its exact `ValueError(`conversion not supported`)`, and `La` retains the earlier validation error. Declared RGBa source pairs no longer eagerly unpremultiply and route through RGB; their native source strategy now owns those conversions. RGB→L retains its dedicated Grayscale operation route. CMYK→I/F no longer materializes an intermediate RGB image. Every accepted P/PA source pair now enters its prepared strategy; `La` to other targets remains an explicit validation error. The generic P/PA wrapper quantizers were removed after this source-reachability review.
- `color/convert/ops.rs` now publishes exact typed target contracts for the shared RGBA, RGBa, La, I;16, scalar, and RGBX source groups. The four exact I;16 identities share `I16_INPUTS`; scalar I/F and RGBX have separate contracts. Overlapping target lists do not widen any source's accepted target set.
- I and F now declare RGBX as an exact target and execute through their mode-owned Default Convert strategies for CPU or SIMD requests; the SIMD request records the Default fallback. Their obsolete wrapper-side scalar dispatcher was removed after auditing all Pillow Convert destinations. LA, RGBA, and RGBa retain their existing lazy prepared RGBX routes. GPU-selected I/F→RGBX retains the wrapper route with a TODO for GPU migration; the full mode-by-architecture parity cases remain pending.
- CMYK, HSV, and YCbCr now reach P/PA through their exact source-mode Default strategies. Their native stored samples are transformed to RGB work samples using the existing color helpers, then WEB-quantized with the selected dither behavior; PA output uses opaque per-pixel alpha. A shared palette helper constructs the exact P/PA raster. P retains the previous recursive RGB conversion's dropped metadata, while PA retains its `ExifOnly` policy. No intermediate RGB raster or wrapper is constructed.
- `color/convert/ops.rs` now declares target-mode admission against the exact
  source identity from `DynamicImage`. The contract keeps the archived
  source-specific target sets for `1`/`L`, `LA`, `La`, `RGBA`, `RGBa`, and the
  remaining source modes; target `1` is prepared instead of returning the prior
  "deferred conversion mode must be materialized first" error. Prepared
  palette contracts cover One/L/LA/I/F and all four I;16 identities→P/PA,
  RGB→P/PA, RGBA→P/PA, RGBa→P/PA,
  RGBX→P/PA, LAB→P/PA, CMYK/YCbCr/HSV→P/PA, P→PA, and PA→P. Remaining source
  pairs and lazy palette materialization still have TODOs. Internal high-depth
  source routes to P/PA use native samples, are marked `NO_PILLOW_SUPPORT`,
  and are reported as core-only.
- The RGB8 source strategy is implemented at `convert/rgb8/default/convert_impl.rs` and declared through `convert/rgb8/ops.rs`. It handles L, LA, La, P, PA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, HSV, I, and F targets from native RGB triplets. It composes the existing same-mode Default grayscale kernel for L/LA/La, directly writes alpha-bearing and scalar carriers, uses shared byte-buffer color transforms for CMYK/YCbCr/HSV without a temporary converted raster, and quantizes RGB bytes directly for P/PA output; PA receives opaque per-pixel alpha.
- RGBA8→P/PA now has prepared Default strategies in
  `convert/rgba8/default/convert_impl.rs`. P runs the archived FASTOCTREE over
  native RGBA samples and returns its RGB palette plus per-entry alpha with
  dropped source metadata. PA runs median-cut over RGB triplets and carries
  original alpha bytes into the native PA raster with the `ExifOnly` wrapper
  policy. Neither path constructs an intermediate RGB image or `Image` wrapper.
- RGBX8→P/PA now has a prepared Default strategy in `convert/rgbx8/default/convert_impl.rs`. It quantizes RGB samples directly from native RGBX storage, retaining source metadata for P and EXIF only for PA; the unused fourth byte is not interpreted as alpha.
- RGBa→P/PA now has a prepared Default strategy in `convert/rgb_premultiplied8/default/convert_impl.rs`. It preserves the archived raw RGB sample quantization after dropping the fourth byte, retains source metadata for P, and uses opaque PA alpha with the `ExifOnly` policy.
- LAB Default execution now also provides routes to L, LA, RGB, RGBA, CMYK, YCbCr, HSV, I, and F in `convert/lab8/default/convert_impl.rs`; its existing identity, I;16, and P/PA routes remain. L/LA use the native LAB grayscale kernel; RGB preserves the raw three-byte carrier; RGBA adds opaque alpha; numeric and color-space outputs use their existing native helpers. P/PA quantize the three-byte LAB carrier directly; P preserves source metadata, while PA uses opaque per-pixel alpha and the `ExifOnly` policy. The new target routes are source-only and unverified; the preexisting I;16 pairs also need exact archived-contract review before closing Convert.
- RGB8→La, RGB8→RGBa, RGB8→RGBX, and RGB8→P/PA are Pillow-supported conversions. The Convert table supplies the first three directly; Pillow's `topalette` route supports RGB input for both P and PA. Their Default and architecture implementations consume native RGB triplets and emit the exact destination variant. SSE2 build availability includes x86_64 and 32-bit x86 compiled with `target_feature="sse2"`; portable x86 builds retain Default fallback.
- RGB8's architecture support declares direct opaque-alpha routes for La and opaque four-channel targets RGBa/RGBX, reusing the LA and RGBA kernels while restoring the exact destination `Mode` variant. This is valid because the appended alpha/padding sample is 255, so premultiplied and ordinary color samples are identical for these conversions. The RGBa/RGBX architecture routes retain the nonempty guard used by RGBA expansion. The target guards expose the reconciled Pillow source/target matrix; same-mode RGB uses the archived five-pixel layout-copy path.
- The Pillow palette support matrix comes from its special [`frompalette`](https://github.com/python-pillow/Pillow/blob/main/src/libImaging/Convert.c#L1021-L1060) and [`topalette`](https://github.com/python-pillow/Pillow/blob/main/src/libImaging/Convert.c#L1086-L1105) paths: P/PA sources use `frompalette`; `topalette` accepts L, RGB, RGBX, RGBA, and RGBa for P and PA. One, LA, CMYK, YCbCr, HSV, LAB, I, F, and the four I;16 sources remain accepted native core extensions where implemented; their P/PA targets are marked CoreOnly in the corresponding support declarations and `NO_PILLOW_SUPPORT` beside their Default palette implementations. The internal five high-depth sources are CoreOnly by their mode scope. This classification is source-only; parity must cover the newly separated Pillow/core-only pairs.
- RGB8 execution is now split into `sse2/convert_impl.rs`, `neon/convert_impl.rs`, and exact Luma8-only AVX2, SSSE3, and Wasm SIMD128 implementations. `rgb8/ops.rs` owns only support declarations, preparation, and architecture orchestration; shared typed output construction and telemetry live in `rgb8/helpers/vector.rs`. No algorithms or output contracts changed in this placement pass.
- RGBA8, RGBa, and RGBX Luma8 architecture routes now invoke their concrete grayscale kernels from their own SSE2, NEON, AVX2, SSSE3, or Wasm SIMD128 strategy files. The former per-mode grayscale helpers that accepted an `Architecture` and matched again have been removed. Source review of the full Convert matrix remains open.
- RGBX-to-RGB keeps its SSSE3 and NEON four-channel-to-three-channel kernels in concrete architecture owners; the RGBX strategy composes those raw-byte kernels while preserving the RGBX input identity until the explicit RGB output is constructed.
- La→LA now has declared nonempty SSE2, NEON, and Wasm SIMD128 strategies over native interleaved two-byte samples. The exact archived unpremultiply arithmetic is statically specialized by channel count in `convert/helpers/alpha.rs` and shared with RGBa's four-byte kernel; alpha and zero-alpha source samples are preserved, and empty inputs select Default. This route is source-reviewed but uncompiled and unvalidated.
- One, L, LA, RGB, RGBA, RGBa, and RGBX now have separate Default source folders and support declarations. One and L consume native luma bytes directly for RGB/RGBA expansion, grayscale LA, direct CMYK/YCbCr/HSV output, typed I/F samples, and all four I;16 target identities. The One strategy expands each nonzero sample to 255 as required by the Pillow `1` contract; L preserves each input byte. LA converts directly from interleaved luma-alpha samples, preserving alpha for RGBA/RGBX and LA outputs without an intermediate RGB raster. RGB consumes native triplets. RGBA handles alpha retention, RGBA-to-RGBa premultiplication, opaque RGBX padding, direct CMYK/scalar output, and YCbCr/HSV conversion directly from four-byte source pixels. RGBa unpremultiplies only for the legacy straight-alpha targets and La conversion; its LA, CMYK, YCbCr, HSV, I, and F paths use the archived raw premultiplied samples. RGBX always treats its fourth sample as padding and creates opaque alpha when converting to RGBA/LA.
- Luma8's archived nonempty SIMD matrix is now declared and routed for LA, RGB, RGBA, CMYK, YCbCr, HSV, I, and F under SSE2/NEON support. The kernels consume L bytes directly, preserve the archived 8-, 16-, or 4-pixel block widths and scalar output semantics, write typed output carriers, and select the exact destination mode. The four I;16 destinations retain their separate archived vector route, including empty-image control behavior. Other empty requests retain the Default route through nonempty guards. SSE2 is build-available on x86_64 and on 32-bit x86 when SSE2 is enabled at compile time.
- Bilevel `1` to each exact I;16 destination now declares SSE2, NEON, and Wasm SIMD128 strategies. The kernel expands native bytes to zero or 255 in 16-sample vectors and constructs the requested I;16 identity directly. Its target guard preserves the archived empty-image route. This source migration has not been compiled or validated.
- Prepared conversion distinguishes the grayscale contract for target `L` from target `LA`: `L` composes the exact source mode's Default grayscale strategy, while `LA` uses Convert's generic grayscale semantics before its alpha-preservation path. This preserves the archived difference for an `RGBa` source, where target `L` unpremultiplies but target `LA` uses the stored premultiplied color samples.
- CMYK8, YCbCr8, and HSV8 have independent support declarations and direct Default source strategies. CMYK derives RGB samples in each final-destination loop and retains the existing scalar arithmetic for L, LA, RGB, RGBA, YCbCr, HSV, I, and F. YCbCr uses its native Y channel for L/LA and directly applies the archived fixed-point RGB transform for color/scalar destinations. HSV uses the archived integer-domain lookup for RGB-derived destinations. Neither strategy materializes an intermediate RGB image.
- P and PA now have independent target contracts and direct Default strategies that receive resolved `PaletteInput`. P preserves palette-provided alpha for LA/RGBA and uses opaque defaults when absent; PA preserves its per-pixel alpha only on alpha-bearing output paths and ignores it for RGB/scalar conversions. Both map palette indices directly into final destination loops without first expanding a whole RGB raster.
- Registry type plumbing exposes the prepared Convert plan through the color-family facade. All 25 source identities now have separate Default strategy owners and support declarations, including LAB's existing identity route and the five internal high-depth storage identities. Luma8 uses the archived L→L 16-byte layout copy, L→LA and L→RGB/RGBA layout kernels, the CMYK/YCbCr/HSV/typed kernels, and all four I;16 widening routes under SSE2 and NEON selections; it now declares Wasm SIMD128 for those same vector targets using the shared portable kernels; its I;16 routes retain the archived zero-block vector receipt; reviewed generic targets now admit exact architecture routes and record scalar-control for empty images. LumaA8 now declares all 11 archived LA target routes under SSE2, NEON, and Wasm SIMD128 mode guards: native layout for L/LA/RGB, premultiplication for La, direct RGBA/RGBX expansion, direct CMYK/YCbCr/HSV, and typed I/F output. It retains native interleaved LA samples and exact result variants; generic empty targets retain the archived scalar-control receipt; the reviewed alpha route zero-block behavior is detailed above. RGB8→RGB retains the archived five-pixel layout-copy route under SSE2, NEON, and Wasm; the source and architecture owner review, including its empty guard, is detailed above. RGB8→Luma8 reuses the exact RGB8 Grayscale AVX2, SSSE3, SSE2, NEON, and Wasm SIMD128 kernels under Convert's typed target strategy; its target guard retains the archived empty-image scalar-control path. RGB8→LumaA8 uses a direct eight-pixel RGB/luma/opaque-alpha kernel under SSE2, NEON, and Wasm, and its Default route materializes the archived rounded L scratch before LA output. RGB8→CMYK uses the archived four-pixel inverse-channel kernel under SSE2, NEON, and Wasm, writing K=0 into the native CMYK carrier. RGB8→YCbCr uses the archived eight-pixel fixed-point table kernel under SSE2, NEON, and Wasm and writes directly to the YCbCr carrier. RGB8→HSV uses the archived eight-pixel float-lane conversion, preserving its tie order and f32 hue narrowing, under SSE2, NEON, and Wasm. RGB8→I/F use the archived eight-pixel typed-lane conversions under SSE2, NEON, and Wasm and write little-endian scalar bytes directly into their native four-byte carriers. RGB8→RGBA8 declares SSE2's archived four-pixel vector path, NEON's archived sixteen-pixel structure-load path, and the portable Wasm path; the empty-image receipt and allocation-failure handling are source-reconciled above. RGBA8 retains SSSE3/NEON specialized RGBA8→RGB8 hot loops, falling back to the portable wide-layout kernel if specialized output reservation fails, and declares the archived four-pixel wide-layout route under SSE2 and Wasm; RGBA identity uses its archived wide-layout route under SSE2, NEON, and Wasm. It reuses the exact RGBA8 Grayscale AVX2/SSSE3/SSE2/NEON/Wasm SIMD128 kernels for RGBA8→L and routes RGBA8→LA/RGBa/CMYK/YCbCr/HSV/I/F through native four-byte vector strategies under SSE2, NEON, and Wasm; RGBA8→RGBX8 uses the archived opaque-padding helper under SSE2, NEON, and Wasm. RGBX8 source declares the archived four-pixel wide layout route for RGB under SSE2 and Wasm alongside its portable LA/RGBA/CMYK/YCbCr/HSV/I/F routes; target-L grayscale and specialized SSSE3/NEON RGB loops stay mode-specific. Each path consumes its prepared selection and preserves the exact output Mode. RGBa native Convert routes use raw premultiplied RGB samples for LA/CMYK/YCbCr/HSV/I/F and preserve source alpha for LA; the archived alpha kernel unpremultiplies for La/RGB/RGBA/RGBX, setting opaque alpha for La and opaque padding for RGBX. The active SSE2/NEON/Wasm declarations compose these exact-mode owners, with nonempty guards for the unpremultiplication routes. RGBa→L reuses the already-existing Grayscale architecture kernels (AVX2/SSSE3/SSE2/NEON/Wasm SIMD128) only for nonempty images; the archived Convert adapter rejects RGBa→L as a vector route, so the maintained guard now selects same-mode Default for empty input. Empty raw color/typed routes retain scalar-control, zero-block receipts; empty alpha transforms fall back to Default. The RGBa target contract now lists all 20 Pillow identities, including its same-mode Default identity route; the public wrapper still uses its `image.copy()` fast path. Cross-mode targets for the five internal high-depth identities remain rejected at `is_convert_target_mode`; same-mode identity is a CoreOnly route that preserves each native raster variant. P/PA identity uses the prepared palette metadata policy and preserves the lazy wrapper palette. CMYK8→RGB declares the archived SSSE3 and NEON kernels plus the portable wide kernel under SSE2 and Wasm SIMD128; all routes process native CMYK samples. The four I;16 source modes now process native u16 values directly under SSE2/NEON/Wasm SIMD128 and retain full native u16 values for I/F; One→I;16 declares the archived 16-sample vector expansion. Archived SIMD Convert source files and their exact vector cases still need a one-to-one review against the maintained source/target declarations, excluding the alpha and RGB identity/layout routes reconciled above; remaining routes include other layout/color-space kernels, same-mode and empty-image guards, architecture build availability, and complete output identity; place retained cases beside their maintained owners and migrate any uncovered path. Do not treat the current dispatcher as a finished support/selection/implementation composition.
- Archived SIMD Convert admission was narrower than the 25-mode typed contract: generic byte-layout conversion recognizes L/LA/RGB/RGBA carriers with either no mode tag or the matching identity, plus RGBA storage explicitly tagged RGBa or RGBX. Special routes add the documented alpha pairs, CMYK→RGB, One/L→I;16, I;16→L/LA/RGB/RGBA/CMYK/I/F, and RGB8→LAB scalar control. Its ordinary vector destinations are L/LA/RGB/RGBA/CMYK/YCbCr/HSV/I/F. Other pairs were not admitted to this vector adapter and used the CPU route. Independently, `Image::convert` retains its same-mode `image.copy()` fast path. The operation contract now lists all 25 same-mode identities, including P/PA palette-preserving identity and CoreOnly same-mode identity for internal high-depth modes. Prepared execution clones the exact `DynamicImage` variant; cross-mode palette outputs remain wrapper-only until their metadata can travel through the lazy terminal. The per-file owner map below records candidate destinations; the remaining zero-size behavior, build availability, and source/target/architecture tuples still require one-to-one review.
- Direct source comparison of `kernels/color_space.rs` with the maintained native Convert strategies reconciles its L/LA branches. Both exact modes read only the native luma sample for these targets and ignore LA alpha, preserving the archived source-channel contract. CMYK emits `[0, 0, 0, 255-L]` in four-pixel groups; YCbCr emits `[L, 128, 128]` in eight-pixel groups; HSV emits `[0, 0, L]` in eight-pixel groups. The active L and LA helpers retain those block widths and exact output identities (`ImageCmyk8`, `ImageYCbCr8`, and `ImageHsv8`); empty executions record scalar-control with zero vector blocks. The direct Luma8 and LA implementations avoid running the generic RGB color-space arithmetic for these neutral inputs while preserving its observable samples. Combined with the RGB-family comparison above, the archived color-space arithmetic and channel-selection review is complete; exact tuple dispatch, guards, and build availability remain under review.
- Direct comparison of `kernels/dispatch/luma.rs` with the active RGBA and RGBX target-L owners and RGB/RGBX target-LA owners reconciles its generic routes: all use the same `19595/38470/7471` fixed-point coefficients and `+32768 >> 16` rounding; LA output writes opaque alpha for RGB/RGBX. Empty native layout conversions are intercepted by the archived adapter's scalar-control branch before this kernel, matching the maintained zero-block scalar-control receipt. RGB→L and RGBa→L use their separate Grayscale strategies. The separate RGBA/RGBa→LA layout route is reconciled below.
- Direct comparison of `kernels/dispatch/typed.rs` with the exact-mode L, LA, RGB, RGBA, RGBa, and RGBX I/F owners reconciles typed projection and packing. L and LA copy the native luma sample into I or widen it directly to f32; LA alpha is ignored. Three- and four-byte RGB-family sources use the archived `19595/38470/7471 + 32768 >> 16` projection for I and `(299R + 587G + 114B) / 1000` for F; RGBA alpha, RGBa stored alpha, and RGBX padding do not participate. Every path writes each scalar with `to_le_bytes`, processes eight pixels per block, pads inactive input lanes with zero, and emits only active lanes. Maintained helpers preserve these projections, little-endian carriers, and eight-pixel tails, while each wrapper constructs its exact `ImageI32` or `ImageF32` output. Empty architecture executions preserve the zero-block scalar-control receipt. The arithmetic, channel selection, endianness, and active-lane review for the archived typed dispatcher is complete; exact tuple dispatch, guards, and build availability remain under review.
- Luma8's architecture table now matches the archived vector-target set exactly: `L`, `RGB`, `RGBA`, `LA`, `CMYK`, `YCbCr`, `HSV`, `I`, `F`, and all four I;16 identities each have SSE2, NEON, and Wasm SIMD128 declarations. SSE2 is build-available on x86_64 or x86 with compile-time SSE2; NEON is AArch64-only; Wasm SIMD128 requires `wasm32` plus the `simd128` target feature. The mode owner routes each declared target to the corresponding concrete architecture file and routes other targets through Luma8 Default. Same-mode `L→L` and ordinary empty vector targets keep their scalar-control, zero-block receipts. The four I;16 targets preserve the archived special receipt: empty input remains a selected vector route with zero blocks. The source/target/architecture declarations, build guards, execution match, and empty-path distinction have been reconciled; cross-operation fallback behavior and final compile evidence remain pending.
- LumaA8's archived vector source/target set is reconciled with its mode owner: `LA→L/LA/La/RGB/RGBA/RGBX/CMYK/YCbCr/HSV/I/F`. The ordinary exact-mode routes are declared for SSE2, NEON, and Wasm SIMD128; `LA→La` alone has a nonempty guard and therefore selects the same-mode Default implementation for zero-pixel input. The archived alpha adapter handles `LA→RGBX` separately from generic target admission; its active LA strategy preserves the fourth source alpha as the RGBX padding byte and retains the archived vector/zero-block receipt for empty input. Other empty architecture targets preserve scalar-control with zero blocks. The active architecture match and build conditions are explicit: SSE2 on x86_64 or compile-time-SSE2 x86, NEON on AArch64, and Wasm SIMD128 on wasm32 with `simd128`. Each exact target constructs its own raster variant; P/PA remain Default palette routes, not vector color-layout aliases. This LA tuple, guard, output-identity, and empty-path review is complete; full matrix parity and compile evidence remain pending.
- The archived `La` alpha route accepts `La→LA`; same-mode `La→La` remains the wrapper's identity copy, and every other ordinary Convert target retains `conversion from La to L not supported`. `LA_PREMULTIPLIED_TARGETS` exposes those two accepted identities and the same rejection text. The maintained Default route preserves the alpha byte while applying the archived exact unpremultiply arithmetic; the SSE2, NEON, and Wasm SIMD128 strategies reuse the channel-specialized 16-byte kernel and emit the exact `ImageLumaA8` variant. Its nonempty target guard reproduces the old alpha adapter's zero-block unsupported result by selecting Default for empty input. The active build guards require x86 SSE2, AArch64 NEON, or wasm32 `simd128`, respectively. This exact source/target, mode error, kernel width, output, architecture, and empty fallback review is complete; final compile/parity evidence remains pending.
- Direct comparison of `kernels/dispatch/layout.rs` and `kernels/layout.rs` with the active exact-mode owners reconciles all native byte-layout routes. Generic layout groups use `16 / max(source_channels, target_channels)` pixels per block: L→LA uses eight, LA→L/LA uses eight, LA→RGB uses five, RGB identity uses five, RGB→RGBA uses four, and RGBA/RGBX→RGB or RGBA uses four. The active owners preserve the archived channel maps: L→RGB/RGBA replicates luma and inserts opaque alpha; LA→L drops alpha, LA→LA preserves both bytes, and LA→RGB replicates luma; LA→RGBA uses the archived 16-pixel four-shuffle expansion and preserves alpha; LA→RGBX preserves alpha in its fourth byte. RGB→RGBA appends opaque alpha; RGBA identity copies all four bytes; RGBA/RGBX→RGB drops the fourth byte; RGBX→RGBA inserts opaque alpha. RGBA/RGBa→LA uses the archived `native_rgba_to_la_bytes` block: the active RGBA helper retains its `77/150/29` plus residual-shift arithmetic, and the RGBa helper's `19595/38470/7471 + 32768 >> 16` expression is algebraically identical to those staged shifts. Both preserve alpha and RGBa's stored premultiplied color lanes; neither unpremultiplies. The archived 16-pixel input grouping and eight-pixel output block count are preserved, and zero-pixel execution records scalar-control. Empty RGB→RGBA NEON execution records the archived scalar-control receipt. On NEON reservation failure, RGB→RGBA now falls through to the shared portable four-pixel layout helper; that helper uses the archived generic `Vec::with_capacity` allocation behavior. Exact architecture declarations and build availability remain part of the overall Convert audit.
- RGBX→La and RGBX→RGBa are wrapper-accepted core extensions but absent from
  Pillow's [Convert table](https://github.com/python-pillow/Pillow/blob/main/src/libImaging/Convert.c#L1445-L1453).
  Their `RGBX_INPUTS` pairs are explicitly core-only, with `NO_PILLOW_SUPPORT`
  beside their implementation. Direct Default strategies preserve the current
  wrapper's exact samples: native BT.601 luma with opaque alpha for La, and
  native RGB with opaque alpha for RGBa. SSE2, NEON, and Wasm SIMD128
  declarations reuse RGBX's existing architecture-selected opaque LA and RGBA
  kernels and construct the exact premultiplied DynamicImage variants. The
  wrapper's mode-support check now admits these routes to prepared execution;
  no intermediate RGB conversion is needed. This source migration is
  unverified.
- Archived SIMD Convert file map (candidate destinations; guard review pending):

  | Archived source | Maintained owner or reference-only disposition |
  | --- | --- |
  | `pool_cpu/legacy/pool_simd/ops/adapters/convert.rs` | `pool_cpu/ops/color/convert/ops.rs`, exact source-mode `convert/<mode>/ops.rs`, `color/ops.rs`, and `ops/convert/dispatch.rs`; reconcile admission, scalar-control, and empty-input decisions. |
  | `pool_cpu/legacy/pool_simd/ops/adapters/convert/alpha.rs` | `convert/helpers/alpha.rs` plus `luma_a8`, `la_premultiplied8`, `rgba8`, `rgb_premultiplied8`, and `rgbx8` mode/architecture owners; source/target mapping, the LA nonempty `La` guard, and empty receipts are reconciled above; the remaining exact architecture tuple and build audit is tracked in the broader matrix review. |
  | `pool_cpu/legacy/pool_simd/ops/adapters/convert/cmyk.rs` | `convert/cmyk8/{sse2,ssse3,neon,wasm_simd128}/convert_impl.rs`; source route, arithmetic, packing, block boundaries, and active-lane handling reviewed above. |
  | `pool_cpu/legacy/pool_simd/ops/adapters/convert/kernels/color_space.rs` | `convert/helpers/native_rgb.rs` plus L, LA, RGB, RGBA, RGBa, and RGBX mode/architecture owners; RGB and L/LA arithmetic, native sample selection, and output identities are reconciled above. Exact source/target/architecture tuples, guards, and build availability remain in the overall Convert audit. |
  | `pool_cpu/legacy/pool_simd/ops/adapters/convert/kernels/dispatch.rs` | Per-mode `convert/<mode>/ops.rs` architecture matches; account for every accepted kernel and same-mode Default fallback. |
  | `pool_cpu/legacy/pool_simd/ops/adapters/convert/kernels/dispatch/layout.rs` | All native byte-layout maps, per-block widths, and active output lengths are reconciled above; exact architecture declarations and build availability remain in the overall Convert audit. |
  | `pool_cpu/legacy/pool_simd/ops/adapters/convert/kernels/dispatch/luma.rs` | RGBA/RGBX→L and RGB/RGBX→LA generic arithmetic plus empty scalar-control guard are reconciled above. RGB/RGBa→L use separate Grayscale strategies; RGBA/RGBa→LA's specialized layout block is reconciled above. |
  | `pool_cpu/legacy/pool_simd/ops/adapters/convert/kernels/dispatch/typed.rs` | Native I/F projection, little-endian packing, and active-lane handling are reconciled above; exact source/target/architecture tuples and build availability remain in the overall Convert audit. |
  | `pool_cpu/legacy/pool_simd/ops/adapters/convert/kernels/layout.rs` | Native byte mappings, RGBA/RGBa→LA arithmetic/packing, empty RGB→RGBA receipt, and the specialized allocation fallback are reconciled above. TODO(#3ptb): finish same-mode/empty guards, build availability, and exact output identity for remaining source/target pairs. |
  | `pool_cpu/legacy/pool_simd/ops/adapters/convert/kernels/mod.rs` | No kernel of its own; reconcile each re-export with the maintained owner map above. |
  | `pool_cpu/legacy/pool_simd/ops/adapters/convert/kernels/rgba_rgb_neon.rs` | `convert/rgba8/neon/convert_impl.rs`; its 16-pixel prefix, scalar remainder, and portable-layout allocation fallback now match the archived route. |
  | `pool_cpu/legacy/pool_simd/ops/adapters/convert/kernels/rgba_rgb_ssse3.rs` | `convert/rgba8/ssse3/convert_impl.rs`; its SSSE3 shuffle blocks, tail copy, and portable-layout allocation fallback now match the archived route. |
  | `pool_cpu/legacy/pool_simd/ops/adapters/convert/tests.rs`, `pool_cpu/legacy/pool_simd/ops/adapters/convert/tests/cmyk_packing.rs` | Read-only behavior references; do not add unit tests. Map each relied-on condition into the frozen parity denominator after the source audit. |
- No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage command has run for this source.

### RemapPalette source progress: incomplete and unvalidated

- `color/ops.rs` declares an exhaustive contract over all 25 identities. Only `ImageLuma8` and `ImagePalette8` are supported; both consume their native one-byte samples and return exact `ImagePalette8`. Every other identity is explicitly rejected with the existing `ValueError("illegal image mode")` contract. The public wrapper already admits only L/P and constructs the reordered palette and alpha metadata separately.
- Both Default mode implementations apply the legacy inverse map directly to the source bytes: `inverse[dest_map[new_index]] = new_index`, with all unmapped values remaining zero. No RGB conversion or retagging is used to share code. The shared kernel is in `color/helpers/remap_palette.rs`.
- L and P declare SSE2, NEON, and Wasm SIMD128 strategies with exact one-byte input contracts. The shared portable-vector kernel preserves the archived 16-byte lookup and padded-tail behavior; the serial Default path remains available for every mode. Rayon is not used. The old independent SIMD registration has been removed; prepared CPU registry dispatch selects these architecture strategies.
- `Image::remap_palette_with_source` retains ownership of palette colors and alpha, while its explicit output mode is P. The migrated strategy returns that exact mode-bearing raster. Map inputs continue to be capped at 256 at the registry boundary, matching the previous callback.
- No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage command has run for this source.

### Resize source progress: prepared pipeline and mode strategies present; source review remains incomplete

- `resize/ops.rs` declares exhaustive ordinary and boxed support contracts over all 25 exact `Mode` identities, selects from the injected CPU `ExecutionContext`, and retains the selection receipt. The prepared dispatcher routes byte-backed modes to exact typed mode owners, I32/F32 to native scalar owners, four I;16 modes to endian-specialized u16 owners, and five internal high-depth modes to native u16/f32 owners. The 14 byte modes use mode-local Default routes: direct sample paths share the fixed-width `byte_samples` helper where their filtering semantics match, LA/RGBA use the straight-alpha helper, CMYK owns its narrow fixed-point loop, and HSV owns its Bicubic narrow fixed-point loop. All 14 byte modes declare exact-mode SSE2, NEON, and Wasm SIMD128 strategies for ordinary nearest requests; eleven byte modes plus Luma8 also declare guarded filtered strategies. Boxed byte nearest and convolution use exact-mode architecture strategies with checked bounds and coefficient admission; One and P retain nearest-only boxed vector routes. Each mode reconstructs its own typed output, and `One` preserves its threshold step. I32/F32 retain their prepared AVX/FMA selections and Default fallbacks. I32 also declares NEON for eligible unboxed nearest and filtered requests; nearest preserves the raw signed-word carrier, while filtering uses checked NEON blocks around the two-pass i32 intermediate and a wide-lane fallback. The four I;16 modes have unboxed wide-vector strategies for SSE2, NEON, and Wasm SIMD128, with byte order selected by the exact mode owner and Default fallbacks retained. Public Resize queues typed `Resize`/`ResizeBoxed` operations; geometry families and Thumbnail prepare nested Resize with the same injected context. Source review confirms that native I;16 Default routes return correctly shaped exact-mode buffers for empty input extents, while the unboxed vector guard routes those extents through its explicit zero-output path. Remaining source review covers boxed float32-bound crop/pass-elision and validation/error ordering, plus the separate GPU Pillow-string compatibility helper. No build or validation has run.

#### Ordinary native-byte vector Resize source map

The archived ordinary vector route is split across these maintained owners:

| Archived source | Maintained destination / disposition |
| --- | --- |
| `legacy/pool_simd/ops/adapters/resize/kernels/u8.rs` | Serial two-pass orchestration in `resize/helpers/native_byte_vector/execute.rs`; `mod.rs` only declares and exports the helper modules. Exact mode dispatch stays in each byte mode's `ops.rs`, and its selected architecture calls the matching `<arch>/resize_impl.rs`. Its feature-gated Rayon scheduling and runtime parallel thresholds are reserved for Parallel CPU. RGB Thumbnail sparse-row elision is retained in its exact prepared Resize child. |
| `legacy/pool_simd/ops/adapters/resize/kernels/u8_convolution.rs` | Native-byte scalar tails, fixed-point conversion, straight-alpha preparation, and wide-lane convolution in `resize/helpers/native_byte_vector/convolution.rs`. |
| `legacy/pool_simd/ops/adapters/resize/kernels/u8_cover_i32.rs` | Checked i32 mode kernels in `resize/helpers/native_byte_vector/bounded_i32.rs`; the one-channel Luma specialization remains in `resize/luma8/helpers/vector.rs`. |
| `legacy/pool_simd/ops/adapters/resize/kernels/u8_plan.rs` | Coefficient slices and the serial horizontal lane plan in `resize/helpers/native_byte_vector/plan.rs`. |
| `legacy/pool_simd/ops/adapters/resize/kernels/u8_nearest.rs` | Exact native-byte nearest sampling in `resize/helpers/native_nearest.rs`. |
| `legacy/pool_simd/ops/adapters/resize/kernels/u8_boxed_convolution.rs` | Serial boxed two-pass plan and row stages in `resize/helpers/native_byte_vector/boxed.rs`; exact mode/architecture wrappers call it from each byte mode's `<arch>/resize_impl.rs`. Its Rayon scheduling and runtime thresholds are reserved for Parallel CPU. Boxed nearest coordinates and packed rows use `resize/helpers/native_nearest.rs`. |

The maintained helper is called only after the prepared owner has matched the
exact mode and selected an architecture. Its channel and straight-alpha policy
are compile-time parameters; it does not inspect `DynamicImage`, infer a mode,
or schedule parallel work. Boxed support facts retain float bounds as exact
bits, and admission applies the same float32 box boundary before checking
dimensions and coefficients. This source migration has not been compiled or
validated.

#### Resize mode source review and destination

The archived CPU entry points are `legacy/ops/resize.rs::{execute_resize,
execute_resize_boxed}` and `legacy/ops/thumbnail.rs::execute_thumbnail` with
`legacy/ops/thumbnail/{fusion,tests}.rs`. They delegate the resampling loops
to `ops/pil_resize`; Thumbnail also composes the legacy Reduce owner. Direct
and boxed Resize preserve source identity except for mode `1`, whose byte
samples are resampled and then thresholded back into the exact `One` output.
Boxed Resize narrows box geometry to float32 before crop/pass-elision checks.
Thumbnail preserves source identity, applies reducing-gap preparation, and
forces nearest for `1` and `P`. The rows below describe the source route and
the required destination; they do not claim the new strategies are complete.

| Exact mode | Archived source behavior / current migration disposition |
| --- | --- |
| `One` | Pillow mode `1`; its exact Default owner resizes the native one-byte plane with serial nearest or fixed-point separable kernels, then thresholds at 128 and emits `One`. Boxed nearest preserves the float32-narrowed cumulative affine coordinates. Thumbnail forces nearest. |
| `ImageLuma8` | Native one-byte luma; its exact Default owner retains the serial i32 coefficient specialization when every ordered byte partial sum is proven safe, and uses the fixed-point scalar byte-plane kernel otherwise. It preserves the standard all-zero fast path and boxed pass-elision/crop behavior. |
| `ImageLumaA8` | Native two-byte LA; for non-nearest resampling, preserve the archived premultiply/resample/unpremultiply order. |
| `ImageLaPremultiplied8` | Native premultiplied LA (`La`); do not premultiply a second time. |
| `ImagePalette8` | Its exact Default owner resamples native palette-index bytes through the serial one-byte plane kernel and never expands indices. Thumbnail forces nearest. |
| `ImagePaletteAlpha8` | Native PA index/alpha byte pairs; preserve the pair without alpha premultiplication. |
| `ImageRgb8` | Native three-byte RGB; preserve the sparse-row Thumbnail fusion and its exact guards. |
| `ImageRgba8` | Native four-byte RGBA; preserve non-nearest alpha premultiplication and its direct-copy nearest path. |
| `ImageRgbPremultiplied8` | Native premultiplied RGBA carrier (`RGBa`); resample stored bytes directly. |
| `ImageRgbx8` | Native four-byte RGBX carrier; byte four is padding, never alpha. |
| `ImageCmyk8` | Native four-byte CMYK; K is a color sample, never alpha; retain its serial integer-coefficient specialization. |
| `ImageYCbCr8` | Native three-byte YCbCr; resample stored channels without a color-space conversion. |
| `ImageLab8` | Native three-byte LAB; resample stored channels without a color-space conversion. |
| `ImageHsv8` | Native three-byte HSV; retain the serial bicubic integer-coefficient specialization and scalar fallback. |
| `ImageI32` | Pillow `I` in its four-byte carrier; ordinary and boxed non-nearest Default Resize decode signed-i32 samples, round each pass with `ROUND_UP`, and construct the exact I variant. Ordinary nearest retains the archived half-pixel formula; boxed nearest copies source words using the narrowed source box and cumulative affine stepping in its I32-owned strategy. Filtered Default and AVX/FMA requests use the bounded serial streaming ring and fixed eight-tap horizontal path. Eligible unboxed nearest and filtered requests can select the I32 NEON owner. Nearest gathers signed words into wide lanes and writes the original little-endian carrier; filtered Resize preserves the two-pass i32 intermediate and rounding, using checked NEON blocks where eligible and wide-lane blocks for other spans. Its horizontal interleaved-load kernel requires eight stride-two samples with identical weights, while its vertical kernel requires eight contiguous lanes and at most sixteen taps. NEON admission checks the exact four-byte layout and checked dimensions; boxed, empty, and identity requests retain the existing selected route. Complete parity review remains TODO. |
| `ImageF32` | Pillow `F` in its four-byte carrier; native scalar-word nearest copies and ordered f64 filtering retain f32 pass boundaries, non-finite values, eight-tap kernels, tall-image pass order, and exact `ImageF32` identity. The prepared strategy selects the existing scalar FMA implementation from injected AVX/FMA architecture capability; portable Default remains the ordered fallback. |
| `ImageI16` | Pillow `I;16`; its exact Default strategy resamples native u16 samples with the little-endian byte rule. Unboxed Resize also selects the shared wide-vector data plane through exact SSE2, NEON, or Wasm SIMD128 mode/architecture owners; boxed requests stay on Default. |
| `ImageI16L` | Pillow `I;16L`; its exact Default strategy resamples native u16 samples with the little-endian byte rule. Unboxed Resize also selects the shared wide-vector data plane through exact SSE2, NEON, or Wasm SIMD128 mode/architecture owners; boxed requests stay on Default. |
| `ImageI16B` | Pillow `I;16B`; its exact Default strategy resamples native u16 samples with the big-endian byte rule. Unboxed Resize also selects the shared wide-vector data plane through exact SSE2, NEON, or Wasm SIMD128 mode/architecture owners; boxed requests stay on Default. |
| `ImageI16N` | Pillow `I;16N`; its exact Default strategy resamples native u16 samples with Pillow's historical big-endian rule. Unboxed Resize also selects the shared wide-vector data plane through exact SSE2, NEON, or Wasm SIMD128 mode/architecture owners; boxed requests stay on Default. |
| `ImageLumaA16` | Core-only typed u16 LA extension; ordinary and boxed filtered Resize premultiplies luma by alpha, filters native samples, rounds with `ROUND_UP`, saturates to the u16 range, unpremultiplies, and preserves exact mode. Nearest copies samples. Marked `NO_PILLOW_SUPPORT`. |
| `ImageRgb16` | Core-only typed u16 RGB extension; ordinary and boxed Resize filter native samples per channel, round with `ROUND_UP`, saturate to the u16 range, and preserve the exact mode. Marked `NO_PILLOW_SUPPORT`. |
| `ImageRgba16` | Core-only typed u16 RGBA extension; ordinary and boxed filtered Resize premultiplies color by alpha, filters native samples, rounds with `ROUND_UP`, saturates to the u16 range, unpremultiplies, and preserves exact mode. Nearest copies samples. Marked `NO_PILLOW_SUPPORT`. |
| `ImageRgb32F` | Core-only typed f32 RGB extension; ordinary and boxed Resize filter native samples per channel in f64 and store f32 intermediates/results while preserving the exact mode. Marked `NO_PILLOW_SUPPORT`. |
| `ImageRgba32F` | Core-only typed f32 RGBA extension; ordinary and boxed filtered Resize premultiplies color by alpha, filters native samples in f64 with f32 intermediates/results, unpremultiplies, and preserves exact mode. Nearest copies samples. Marked `NO_PILLOW_SUPPORT`. |

No mode rejection was found in the legacy generic Resize entry point. Both operations classify every identity and derive prepared selection from the operation declaration. The prepared dispatcher matches the exact `Mode` tag and sends the 14 byte-backed identities to their corresponding typed `DynamicImage` owners; the remaining eleven identities go to the scalar or high-depth owners. Every prepared route now forwards the captured context and concrete selected architecture to its mode owner. The I;16 vector route and I32/F32 FMA routes are connected to their current declarations; filtered unboxed I32 now also has an exact NEON declaration and implementation. Ordinary nearest, ordinary filtered convolution, boxed nearest, and boxed filtered convolution now have corresponding exact byte-mode architecture owners; prepared caller and validation review remains open.
Mode folders belong only to modes with executable Default strategies; architecture folders belong only to retained kernels. Resize and ResizeBoxed are owned by `resize`; Thumbnail is a separate family and must inject the same execution context into its nested Reduce/Resize work. The five core-only high-depth routes above must not use byte-materializing adapters. Rayon scheduling and the generic compatibility path's runtime FMA selection remain explicit migration items, not part of serial CPU Default selection.

`pool_cpu/ops/resize/ops.rs` prepares ordinary and boxed Resize, retains the support selection, and routes exact modes to native owners. CMYK and HSV now have mode-local narrow fixed-point kernels; HSV uses the retained specialization for Bicubic and native three-channel filtering otherwise. One keeps threshold behavior, straight-alpha modes preserve their alpha contract, palette modes preserve indices, and color/padding modes preserve stored channels. Typed I32/F32 and high-depth implementations remain native. The four I;16 modes now have const-specialized endian rules in both Default and wide-vector unboxed paths; empty sources use the zero-output vector path when its guard is satisfied. I32 filtered unboxed NEON has an exact mode/architecture owner and guarded support declaration. All 14 byte modes now have exact SSE2, NEON, and Wasm SIMD128 declarations for guarded ordinary nearest sampling; the shared kernel retains 16-byte shuffle windows, scalar gathers, bounded output stores, and scalar tails. Eleven byte modes plus Luma8 have guarded ordinary filtered vector strategies, and all byte modes have boxed nearest strategies; filtered boxed convolution is declared for all but the One/P nearest-only routes. Remaining Resize work is eager/nested caller context injection, boxed-bound validation and legacy error-order review, and old direct `ops/pil_resize` caller migration. No build or validation has run.

### Thumbnail source review: complete; behavior verification pending

- `thumbnail/ops.rs` exposes the prepared CPU operation and exhaustively lists
  all 25 `Mode` identities. It preserves source identity and passes the same
  `ExecutionContext` into nested Reduce and Resize plans. Preparation selects
  one exact-mode policy: `1` and `P` force nearest, `LA`, `La`, `PA`, and
  `RGBA` skip reducing-gap while preserving the requested filter, and F32
  retains the eligible unboxed Resize route. Preparation now stores the
  effective filter, reducing-gap choice, factors, and unboxed/boxed resize
  choice in the plan. The F32 rule uses the already selected exact mode and
  dimensions; it does not re-match the materialized raster. These rules are in
  their operation-owned mode folders instead of carrier or string-based
  inference. The top-level exhaustive classifier now sources explicit
  `ModeSupport` declarations from the corresponding `one`, `luma_a8`,
  `la_premultiplied8`, `palette8`, `palette_alpha8`, `rgb8`, `rgba8`, `i32`,
  and `f32` mode owners.
- RGB PutPixel/Thumbnail fusion is routed before CPU batch materializes a
  contiguous RGB write prefix. Its exact RGB implementation applies pending
  writes during the native 2x2 reduction and carries sparse active-row facts to
  the prepared Resize child. Default uses the boxed RGB sparse-row byte helper;
  SSE2, NEON, and Wasm SIMD128 use the exact prepared native Resize strategy
  when available, retaining the archived unboxed vector coefficients and
  sparse horizontal/vertical row elision. If that strategy falls back to
  Default, the prepared boxed ResizeBoxed selection remains the fallback. Dense
  reductions retain the normal selected child path. Source comparison preserves
  reduction, write order, filter, bounds, and zero-row guards; the path is still
  uncompiled and parity- and telemetry-unverified.
- The archived streamed ImageI32 2x2 reduction/resampling path now lives in
  `thumbnail/i32/helpers/streaming.rs`, with PortableFma in the exact Default
  owner and injected AVX/FMA execution in the architecture owner. A typed
  request guard admits the AVX/FMA specialization only for non-nearest filters,
  even source dimensions, an exact 2x2 reducing gap, and valid native I32
  storage. Thumbnail preparation records this path and dispatches directly to
  the mode owner; other requests retain the existing nested Reduce/Resize
  composition. The stream shares the existing I32 Resize coefficient, row,
  and rounding primitives. Manual comparison with the archived loop confirms
  the same mode eligibility, 2x2 reducing-gap calculation, source sample
  decoding, wrapping quartet order, per-pass rounding, coefficient mapping,
  and FMA accumulation order. The result is now constructed directly as
  `ImageI32`; the archived wrapper retagged its four-byte carrier afterward.
  This comparison is source review only. The archived general ImageI32
  Thumbnail reducer performs the same scalar wrapping pair/quartet accumulation
  as ImageI32 Reduce, then passes eight completed values through
  `i32x8::new(values).to_array()` without vector arithmetic. Keep the exact I32
  Default reducer instead of adding a redundant architecture folder. F32's
  ordered even 2x2 vector reduction now runs under the exact ImageF32 Reduce
  strategy when each output row has eight samples; other F32 requests retain
  its native Default path. Manual source review now covers the remaining
  archived branches: `1` and `P` force nearest; `LA`, `La`, `PA`, and `RGBA`
  skip reducing-gap; `F` and `I` use exact typed scalar-mode Reduce/Resize
  owners; and all remaining identities retain native mode through the same
  composed owners. There are no active callers of the archived Thumbnail
  adapter.
- Thumbnail's operation-level declaration links Reduce, Resize, and
  ResizeBoxed support. The prepared operation retains concrete child
  selections for composition paths; the direct I32 stream retains its exact
  selected owner. These receipts are available for the architecture matrix.
- The all-mode × declared-architecture Thumbnail parity matrix and telemetry
  review remain pending. No build, formatter, checkstyle, lint, parity,
  benchmark, or coverage has run for this source progress.

#### Resize caller and wiring audit

- `compute/registry/registration/geometry.rs` now registers Resize and
  ResizeBoxed through `resize::ops::prepare_pipeline`. Their callback no longer
  accepts a Pillow string or returns a direct result. Thumbnail is a separate
  prepared family; it carries the exact `Mode` and the same `ExecutionContext`
  into its nested Reduce/Resize strategies. The RGB PutPixel fusion is wired in
  the serial CPU batch executor through `thumbnail/rgb8/ops.rs`; it passes the
  exact prepared boxed Resize architecture selection and reducer active-row
  facts into the RGB mode owner. The archived streamed ImageI32 route now
  enters its exact mode/architecture owner; other archived vector kernels
  remain pending.
- `PreparedOperation` and `OperationId` expose the Resize, ResizeBoxed, and
  Thumbnail contracts. Thumbnail registration now targets
  `pool_cpu::ops::thumbnail::ops::prepare_pipeline`; the old `ops::geometry`
  callback is no longer the active route.
- `ops/pil_resize.rs` remains a shared facade for coefficient, filter, FMA,
  sample-conversion, and row-pass helpers used by exact mode owners. Its
  generic full-operation Resize wrappers are not used by active prepared CPU
  callsites. The I;16 empty-output boundary still needs source review: the
  generic compatibility wrapper returns before dispatching to the exact typed
  mode strategies.
- Ordinary and boxed I;16 paths now call the four exact typed mode strategies.
  Their mode implementations fix the byte-order rule and construct the exact
  output variant; a shared native-u16 helper contains the arithmetic and does
  not infer mode or convert image storage. GPU still consumes
  `luma16_resample_big_endian(Option<&str>)` from
  `compute/pool_gpu/resize/luma16.rs`; migrate that compatibility boundary
  when shared metadata routing is updated, without adding codec-owned Pillow
  mode metadata.
- Exact CPU mode owners call shared low-level Resize primitives instead of the
  generic full-operation `pil_resize` wrappers. Public `Image::resize` queues
  typed pipeline operations; Contain/Cover/Fit/Scale, Pad, and Thumbnail bind
  their children through prepared Resize plans and retain the injected
  context. The legacy sparse RGB adapter and its sparse row-pass branch have
  been removed; the prepared RGB fusion now owns its selected boxed or vector
  path. The remaining public Resize review is boxed float32-bound validation
  and error ordering. Archived ImageOps.fit behavior remains a guard: force
  nearest for P only, while PA retains the requested filter.
- `ops/resize/reducing_gap.rs::apply_reducing_gap` already accepts typed `Mode`
  and explicitly rejects all four `I;16*` identities when integer reduction
  is required. Preserve its safe-box calculations and verify public error
  ordering against the archived behavior. GPU still consumes
  `luma16_resample_big_endian(Option<&str>)` from
  `compute/pool_gpu/resize/luma16.rs`; migrate that separate compatibility
  boundary when GPU mode metadata routing is reviewed.
- The resampling facade is also consumed outside Resize: `compute/pool_gpu`
  uses its coefficient, filter, integer rounding, typed-sample, and I;16 byte
  helpers; `ops/convert/dispatch.rs` and the folded CPU grayscale
  premultiplied-alpha strategies use its alpha helpers; and Reduce's shared
  scalar arithmetic uses `round_up`. Preserve these cross-family/backend
  dependencies through correctly scoped shared helpers instead of moving
  them wholesale into a CPU mode folder.
- No build or validation has run. The call-site search is a migration map,
  not proof that Resize or Thumbnail is complete or that any exported helper
  is dead code.

### Grayscale source review: complete, unvalidated

- The operation classifies all 25 exact source identities, rejects the
  unsupported `ImageLaPremultiplied8` conversion with the preserved error, and
  routes registered execution through its mode-owned prepared strategies.
- The wrapper's typed mode planner predicts `ImageLuma8`; eager RGB and lazy
  P/PA materialization call the prepared CPU route. Convert's target-L path
  composes the same mode's Default grayscale leaf because Convert has no
  grayscale architecture kernel of its own.
- The unused `execute_grayscale` façade was removed after a source-wide
  reference search. The source review did not run a build, formatter,
  checkstyle, lint, test, parity, benchmark, or coverage command.

### Explicit pending-route TODO ledger

The following maintained source TODOs are open migration routes, not support
claims or completion evidence. Each TODO stays beside its current owner; the
baseline source groups are recorded in Sections 2 and 8. A route is removed
from this table only after its maintained implementation and callers have been
reviewed. New gaps found during that review must be added here and marked with
a source TODO before moving on.

Core support is permitted to exceed Pillow. Supported internal high-depth
identities are core extensions, not Pillow parity cases; their family support
descriptors report `CoreOnly`, and their operation owners carry nearby
`NO_PILLOW_SUPPORT` comments. Existing TODOs for typed native semantics remain
open until their owner-level review removes any conversion used only to share
an implementation.

| Maintained TODO location | Pending route |
| --- | --- |
| `compute/pool_cpu/ops/color/convert/ops.rs` | Complete the one-to-one archived SIMD source/target/guard reconciliation; place every retained kernel under its exact mode/architecture owner or record why its path is unreachable; resolve any uncovered routes; preserve same-mode Default fallback and legacy target/error/allocation contracts; finish prepared registry and public caller review. |
| `ops/convert/operation.rs` | Separately route matrix Convert through a GPU-owned matrix strategy when GPU declares it; the current typed eager wrapper deliberately executes this CPU-owned operation on CPU while GPU support remains outside the CPU migration. |
| `compute/pool_cpu/ops/composite/ops.rs`, `compute/pool_cpu/ops/draw/ops.rs`, `compute/pool_cpu/ops/paste/ops.rs`, `compute/pool_cpu/ops/transform/ops.rs` | Keep each family source/caller review beside its operation owner: finish Composite and Transform source review; finish Draw mode/error review and direct Bitmap/text routes; map retained Paste SIMD cases into parity and complete its operation/caller source review. `compute/pool_cpu/ops/mod.rs` only exports operation families. |
| `compute/pool_cpu/ops/geometry/pad/ops.rs` | Move any remaining archived full-width convolution modes into exact mode/architecture Resize owners, and finish checking every padded mode against its same-mode Default fallback and prepared context. |
| `compute/pool_cpu/ops/enhance/ops.rs` | Contrast source review is complete: nine remaining identities reach Blend's preserved wrong-mode rejection, five internal high-depth identities reject while converting the constant base, and La/RGBa retain their exact conversion errors. Sharpness source review traces wrapper→registry→injected context→exact mode→architecture, confirms exact output identity for all 22 accepted modes and archived native behavior for the six Pillow-supported byte modes, and records explicit `NO_PILLOW_SUPPORT` markers on the other 16 native extensions. The six byte-mode architecture lists and dispatches cover all seven declared entries. Keep Sharpness in the frozen CPU parity matrix after source migration is complete. Expand parity across every operation × exact Mode × Architecture. |
| `compute/pool_cpu/ops/filter/ops.rs` | Compare the migrated Gaussian/box blur vector kernels and RGBA uniform proof against every archived mode route; review every Filter5x5 Default path and direct/eager Filter caller. Native-byte extrema/rank strategies and L/F32 rank vector strategies are represented in their exact mode owners. |
| `compute/pool_cpu/ops/resize/ops.rs`, `compute/pool_cpu/ops/resize/{i32,f32}/ops.rs`, `compute/registry/registration/geometry.rs`, `ops/pil_resize/{resize,boxed,luma16,float}.rs` | The prepared path now preserves float32-bound identity/crop shortcuts and unchanged-axis float pass elision; native I;16 Default and vector paths preserve exact output identity for empty source extents. Still review boxed validation/error ordering, and remove or isolate uncalled generic full-operation wrappers that infer mode/select runtime features while retaining low-level helpers used by strategies; move any retained Rayon scheduling to `pool_parallel`; keep the GPU Pillow-string helper as separate migration debt. Public direct and nested geometry callers already enter prepared plans with injected context. All 14 byte modes declare ordinary nearest vector strategies. ImageLuma8 and eleven other byte modes declare unboxed filtered SSE2, NEON, and Wasm SIMD128 strategies when both coefficient axes satisfy the checked i32 bound; One and P remain nearest-only. Pad composes the exact ImageLuma8 strategies into its output window. All byte modes declare boxed nearest strategies, with filtered boxed convolution for all except One/P. I32/F32 filtered Resize and all four unboxed I;16 vector routes are declared with their Default fallbacks; I32 also declares NEON for unboxed nearest. Boxed I;16 remains Default. Thumbnail prepares nested Reduce/Resize work with the same context. |
| `compute/pool_cpu/ops/eval/ops.rs` | Compare the archived SIMD vector/admission paths against the prepared CPU strategies and migrate any missing mode/architecture case. Move archived Rayon scheduling to `pool_parallel`. Direct scalar point and float-LUT calls enter declared exact-mode CPU strategies. |
| `compute/pool_cpu/mod.rs`, `compute/pool_cpu/batch_execution.rs`, `compute/execution/terminals.rs` | Serial prepared dispatch, routed-context plumbing, wrapper-resolved P/PA palette handoff, ordered PutPixel, transpose-run, and byte-point fusion, architecture receipts for prepared fused work, and CPU architecture terminal receipts are present. Migrate the remaining source-accounted fusions; then finish admission, eager/lazy compatibility, and nested context routes. |
| `compute/mod.rs` | Source-complete: the independent SIMD pool and adapter re-exports are removed. Keep `Backend::Simd` only as a request alias for CPU plus detected Architecture. |
| `compute/execution.rs` | Source-complete: pipeline selection and execution use the injected CPU architecture context; mixed SIMD segments, guessed mode transitions, and retry routing are removed. |
| `compute/automatic_segments.rs` | Removed: CPU mode/architecture strategies now execute the complete prepared operation sequence without a separate segment planner. |
| `compute/eager.rs` (module TODO) | Tiny-RGB ExtractBand now uses the prepared CPU architecture executor for both CPU Default and compatibility SIMD, and records its selection; the generic result-only receipt helper is test-only. Audit remaining direct eager operations for paths that bypass prepared CPU mode/architecture owners, then route any gaps through the injected context. |
| `compute/pool_cpu/ops/put/ops.rs`, `image/data_access/put.rs`, `image/pixel_access.rs` | Prepared PutPixel/PutData/PutAlpha/PutAlphaData callbacks are consumed by the serial CPU batch path; the contiguous RGB PutPixel+Thumbnail fusion is wired. Immediate PutData callbacks now select the exact-mode CPU Default contract and record a receipt because each write must be visible before the next host value is coerced. Numeric I/F and direct I;16 PutPixel use their exact-mode value-at contract and mode-owned CPU Default writers. Finish wrapper metadata and cross-mode conversion review; move the raw-byte I;16 PutData fast path into typed mode strategies with the context described by its adjacent TODO. |
| `compute/pool_cpu/ops/put/ops.rs` | Audit the noted cross-mode conversions; implement native-mode paths or retain only conversions required by the operation's Pillow semantics. |
| `compute/registry/registration/imageops.rs` | Pad uses the prepared geometry owner. Contain, Cover, Fit, and Scale use the Geometry owner and nested prepared Resize plans; Expand uses its prepared CPU owner. Complete the archived Pad mode/kernel review and verify all direct callers use the injected context. |
| `compute/registry/registration/simd.rs` | Removed from the active tree. Compare all archived adapter groups with the assigned CPU mode/architecture owners and migrate any missing strategy or fallback. |
| `compute/pool_cpu/ops/crop/ops.rs` | Expand parity across every exact Mode × available Architecture, including archived SIMD admissions, Default fallbacks, I;16 vector blocks/tails, and CropBorder empty/error contracts. The archived source comparison is complete; the ordinary byte row-copy algorithm is the mode-owned Default `ByteCrop`. |
| `compute/pool_cpu/ops/color3dlut/ops.rs` | Expand parity across Pillow source modes, target modes, channel counts, and available architectures, including native-source core-only modes, the eight-pixel Default fallback, vector tails, and rejected LUT shapes. The archived CPU/SIMD comparison is complete; active Default strategies consume native sample carriers. |
| `compute/pool_cpu/ops/imageops/ops.rs` | Finish direct/fusion caller reconciliation for Invert, Posterize, Solarize, Equalize, Autocontrast, Colorize, CropBorder, and Expand; compare remaining Invert, histogram, and Expand routes. Colorize now supports Pillow L only and does not project other modes through L. Invert's LA/PA/RGBA/CMYK routes are declared CoreOnly. P Posterize/Solarize strategies are now CoreOnly and preserve palette-index identity; target-only parity remains pending. |
| `compute/pool_cpu/ops/imageops/ops.rs` (`prepare_crop_border`) | Expand parity for CropBorder's outer and composed Crop selections across exact modes and architectures, including I;16 guard fallbacks and empty outputs. Source review confirms width-before-height errors and the archived ordinary-byte admission. |
| `compute/pool_cpu/ops/transpose/ops.rs` | Native-byte transpose, including tiled/gathered axis swaps and same-dimension methods, is declared for the archived native-byte and I;16 identities under SSE2, NEON, and Wasm SIMD128; RGBA AVX2 remains preferred for eligible axis swaps. Keep archived parallel collection kernels for the later `pool_parallel` stage; retire the duplicate byte-Flip compatibility route after callers migrate. |
| `compute/pool_cpu/ops/thumbnail/ops.rs` | The Thumbnail operation contract now links conditional Reduce, Resize, and ResizeBoxed support, and the prepared receipt records the exact selected child strategies from the injected context. Continue the archived vector Thumbnail mode/kernel comparison and retire compatibility callers after source review. The streamed ImageI32 2x2 path now has exact Default and guarded AVX/FMA owners. Mode-specific nearest-filter rules for `1`/`P`, reducing-gap exceptions for `LA`/`La`/`PA`/`RGBA`, and the F32 unboxed Resize policy now live in mode-owned strategy folders. |
| `compute/pool_cpu/ops/rotate/ops.rs` | The maintained owner declares all 25 exact modes with serial Default strategies, including typed native `u16`/`f32` paths for the five internal identities, each marked `NO_PILLOW_SUPPORT`. The serial three-byte contiguous-copy and RGBA unrolled nearest paths are restored. All byte-backed exact modes declare native-byte nearest strategies for AVX2, SSE2, NEON, and Wasm SIMD128, including raw-byte `I`/`F` carriers; byte modes declare their archived bilinear vector kernel where supported, including typed I/F arithmetic. Public pipeline calls are connected through the Rotate registry callback and CPU batch executor with injected context. Remaining work is per-mode direct/compatibility caller and archived-behavior source review; core-only extensions stay outside Pillow parity. |
| `compute/registry/registration/imageops.rs` | Source-review direct and eager ImageOps callers against the prepared CPU registry; grayscale's bounded RGB eager path already uses the prepared plan and injected routed context. |
| `image/materialization.rs` | Keep typed prepared plans flowing through the serial pool and preserve resolved operation dependencies on wrapper-to-pool routes. Single-operation lazy P/PA grayscale now prepares before wrapper-wide expansion. |
| `image/construction.rs`, `image/mode_boundary.rs`, `image/pixel_access.rs` | Convert remaining core string-taking constructors, generated-image and pixel-mutation entry points to typed `Mode`; keep Pillow string parsing at host/API boundaries. |
| `image.rs` | Remove the remaining fallback mode identity and reconcile exact materialized and cached pipeline mode ownership. |
| `image/metadata.rs` | Complete direct exact-mode projection for every supported encoded header before decoding, while keeping `DynamicImage` authoritative after materialization. |
| `ops/filter.rs` | Classify filter support from exact `Mode` identities. |
| `compute/pool_cpu/ops/filter/ops.rs`, `compute/pool_cpu/ops/filter/luma8/ops.rs` | Compare migrated Gaussian/box blur and byte-rank vector kernels against their archived routes; review Filter5x5 Default and L rank-window paths; reconcile all direct/eager Filter callers; finish archived mode/kernel review and direct-caller reconciliation. No separate SIMD registrations remain active. F32 rank vector strategy is represented in `f32/ops.rs`. |
| `ops/imageops/point.rs::mode_identity` | Remove the materialization fallback after all lazy pipelines carry an exact cached `Mode` through admission and transitions. |
| `ops/param_filters.rs` | Keep parameter-filter mode selection on `known_mode_identity()` and typed strategy dispatch. |
| `ops/resize.rs` | Pass exact `Mode` identity through the existing resampling implementation. |
| `pipeline/support.rs` | Replace remaining mode string metadata and compatibility parsing with typed `Mode` at host boundaries. |
| `pipeline.rs` | Merge now stores exact typed `Mode`; remove the GPU source-mode snapshot after GPU parameter encoding uses the prepared mode transition. |
| `compute/pool_cpu/ops/color3dlut/ops.rs`, `compute/pool_cpu/ops/imageops/ops.rs` | Color3DLut archived source comparison is complete; expand its operation × source Mode × target Mode × Architecture parity. ImageOps direct wrappers, fusion callers, histogram, Expand, and owned-buffer Invert routes are source-reviewed; operation × exact Mode × Architecture parity remains pending. No separate SIMD compatibility route remains active. |
| `compute/pool_cpu/ops/eval/ops.rs` | Source comparison is complete; expand parity across exact Mode × Architecture and LUT/tail cases. Move the archived Rayon row scheduler to `pool_parallel` in its later backend pass. |

## 8. Baseline exported function declarations

This source scan lists every pub-visible function declaration in maintained baseline CPU and SIMD source, excluding test modules. It includes family entry points and helpers exposed to sibling modules; source visibility alone does not imply runtime reachability. The declared operation-to-family map remains Section 5 of IMPLEMENTATION_STRUCTURE.md, while the caller-reference list above identifies pool-level production callsites. Preserve or retire each listed declaration only after its consumers and feature/architecture configurations have been accounted for.

### CPU (313 declarations across 73 files)

- pillow-rs/src/compute/pool_cpu/batch_execution.rs: 7: execute_batch
- pillow-rs/src/compute/pool_cpu/ops/chops.rs: 326: op_chops_add, 344: op_chops_subtract, 361: op_chops_multiply, 365: op_chops_screen, 374: op_chops_darker, 378: op_chops_lighter, 382: op_chops_difference, 389: op_chops_overlay, 393: op_chops_hard_light, 400: op_chops_soft_light, 407: op_chops_add_modulo, 414: op_chops_subtract_modulo, 421: op_chops_logical_and, 430: op_chops_logical_or, 439: op_chops_logical_xor, 448: op_chops_constant, 457: op_chops_offset, 461: op_chops_duplicate, 465: op_chops_invert
- pillow-rs/src/compute/pool_cpu/ops/color.rs: 18: op_convert, 397: op_convert_lab, 403: op_remap_palette, 451: op_extract_band
- pillow-rs/src/compute/pool_cpu/ops/color3dlut.rs: 17: op_color3dlut
- pillow-rs/src/compute/pool_cpu/ops/composite.rs: 43: op_alpha_composite, 98: op_alpha_composite_mirror, 264: op_merge, 381: op_blend_module, 573: op_composite_module
- pillow-rs/src/compute/pool_cpu/ops/crop.rs: 14: execute_crop
- pillow-rs/src/compute/pool_cpu/ops/draw/arcs.rs: 8: draw_clipped_ellipse, 22: draw_arc_on_canvas, 59: draw_chord_on_canvas, 99: draw_pieslice_on_canvas
- pillow-rs/src/compute/pool_cpu/ops/draw/canvas.rs: 20: from_image, 37: into_image, 47: set_alpha_blend_rgb, 53: set_l16_ink_order, 131: l16_draw_uses_big_endian, 140: draw_native
- pillow-rs/src/compute/pool_cpu/ops/draw/clipping.rs: 22: union_intervals, 37: intersect_intervals, 51: clip_intervals, 118: normalize_angles, 136: clip, 140: and, 144: or, 148: arc_clip_state, 226: chord_clip_state, 248: chord_line_clip_state, 274: pie_side_clip_state, 290: pie_clip_state, 313: for_each_clipped_ellipse_span
- pillow-rs/src/compute/pool_cpu/ops/draw/dispatch.rs: 12: draw_op_on_canvas, 221: is_draw_op, 238: execute_draw_batch
- pillow-rs/src/compute/pool_cpu/ops/draw/ellipse.rs: 95: new, 125: next, 180: draw_hline, 208: for_each_ellipse_span, 235: draw_ellipse_segments, 250: draw_ellipse_on_canvas
- pillow-rs/src/compute/pool_cpu/ops/draw/operations.rs: 11: op_draw_line, 27: op_draw_rectangle, 45: op_draw_rounded_rect, 65: op_draw_ellipse, 83: op_draw_circle, 108: op_draw_polygon, 128: op_draw_arc, 150: op_draw_chord, 170: op_draw_pieslice, 190: op_draw_point
- pillow-rs/src/compute/pool_cpu/ops/draw/polygon.rs: 60: draw_polygon_on_canvas
- pillow-rs/src/compute/pool_cpu/ops/draw/primitives.rs: 6: draw_line_on_canvas, 29: draw_horizontal_span, 54: draw_vertical_segment_exclusive, 86: draw_rect_on_canvas
- pillow-rs/src/compute/pool_cpu/ops/draw/rounded_rect.rs: 7: validate_rounded_rect, 26: draw_rounded_rect_on_canvas
- pillow-rs/src/compute/pool_cpu/ops/effects.rs: 13: apply_effect_rows, 43: blend_row, 74: next, 109: with_process_rng, 118: op_effect_spread, 211: effect_spread_mapping, 260: effect_spread_advance_identity_rng, 279: op_effect_noise, 295: effect_noise_values
- pillow-rs/src/compute/pool_cpu/ops/enhance.rs: 64: op_enhance_brightness, 206: op_enhance_contrast, 270: op_enhance_color_saturation, 443: op_enhance_sharpness
- pillow-rs/src/compute/pool_cpu/ops/eval.rs: 116: op_eval
- pillow-rs/src/compute/pool_cpu/ops/filter/blur.rs: 575: execute_gaussian_blur, 602: execute_box_blur, 610: execute_box_blur_xy_with_passes
- pillow-rs/src/compute/pool_cpu/ops/filter/blur/box_rows.rs: 184: blur_box_radius_one_row_ring
- pillow-rs/src/compute/pool_cpu/ops/filter/blur/line.rs: 6: blur_line_step, 87: blur_line, 160: blur_one_row, 189: blur_rows, 238: blur_columns, 267: blur_rows
- pillow-rs/src/compute/pool_cpu/ops/filter/blur/radius_one.rs: 36: blur_luma_radius_one_fractional_line, 127: blur_rgb_radius_one_fractional_line, 222: blur_rgba_radius_one_fractional_line, 325: blur_interleaved_rows_three_passes, 432: blur_la_radius_one_fractional_line, 515: blur_la_rows_three_passes_radius_one, 562: blur_luma_rows_three_passes
- pillow-rs/src/compute/pool_cpu/ops/filter/blur/radius_one/vertical.rs: 6: blur_luma_vertical_radius_one_rows, 70: blur_interleaved_vertical_radius_one_fractional_rows, 125: transpose_interleaved_rows, 152: blur_columns_radius_one_rows
- pillow-rs/src/compute/pool_cpu/ops/filter/convolution.rs: 5: clip8_filter, 23: pillow_kernel_row_3, 34: pillow_kernel_row_5
- pillow-rs/src/compute/pool_cpu/ops/filter/filter3x3.rs: 119: uniform_i32_filter3x3_sample_is_identity, 139: uniform_i32_filter3x3_is_identity, 450: binomial3x3_parameters_match, 455: execute_filter3x3
- pillow-rs/src/compute/pool_cpu/ops/filter/filter5x5.rs: 259: execute_filter5x5
- pillow-rs/src/compute/pool_cpu/ops/filter/filter5x5/int32.rs: 575: uniform_i32_filter5x5_is_identity, 678: filter_5x5_i32
- pillow-rs/src/compute/pool_cpu/ops/filter/filter5x5/int32/bounded_binomial.rs: 11: i32_filter5x5_binomial_kernel_is_applicable, 27: i32_filter5x5_bounded_binomial_is_applicable, 56: i32_filter5x5_bounded_binomial_kernel_is_applicable, 80: filter_5x5_i32_bounded_binomial
- pillow-rs/src/compute/pool_cpu/ops/filter/filter5x5/native_bytes.rs: 12: filter_5x5_byte_row, 91: filter_5x5_byte_rows, 180: filter_5x5_constant_bytes, 203: filter_5x5_uniform_rgba_identity, 232: uniform5x5_parameters_match, 237: rgb_blur_ring5x5_parameters_match, 245: la_single_outlier5x5_parameters_match, 266: filter_5x5_la_single_outlier_rows, 352: filter_5x5_uniform_luma_rows, 428: filter_5x5_uniform_rgba_rows
- pillow-rs/src/compute/pool_cpu/ops/filter/filter5x5/native_bytes/binomial.rs: 1: binomial5x5_parameters_match, 17: filter_5x5_binomial_luma_parallel_rows, 109: filter_5x5_binomial_la_parallel_rows, 211: filter_5x5_binomial_luma_rows, 273: filter_5x5_binomial_la_rows
- pillow-rs/src/compute/pool_cpu/ops/filter/kernels/blur_line_steps.rs: 6: l_step, 32: la_step, 79: rgb_step, 138: interleaved_step, 190: run
- pillow-rs/src/compute/pool_cpu/ops/filter/kernels/i32_cross_smooth.rs: 5: matches, 39: apply
- pillow-rs/src/compute/pool_cpu/ops/filter/kernels/mod.rs: 4: matches, 8: apply
- pillow-rs/src/compute/pool_cpu/ops/filter/rank.rs: 693: execute_median_filter_with_mode, 702: execute_max_filter_with_mode, 711: execute_min_filter_with_mode, 720: execute_rank_filter_with_mode
- pillow-rs/src/compute/pool_cpu/ops/filter/rank/float.rs: 3: native_uniform_f_mode_image, 19: rank_filter_f_requires_legacy_sort, 43: select_rank_filter_f_value, 184: rank_filter_f_impl
- pillow-rs/src/compute/pool_cpu/ops/filter/rgb_blur_ring.rs: 15: parameters_match, 23: apply
- pillow-rs/src/compute/pool_cpu/ops/filter/uniform_box.rs: 17: apply_if_uniform
- pillow-rs/src/compute/pool_cpu/ops/filter/uniform_image.rs: 12: native_small_uniform_byte_image
- pillow-rs/src/compute/pool_cpu/ops/grayscale.rs: 16: op_grayscale
- pillow-rs/src/compute/pool_cpu/ops/imageops/colorize.rs: 3: gather_native_channel, 79: colorize_lut, 136: op_colorize
- pillow-rs/src/compute/pool_cpu/ops/imageops/expand.rs: 122: op_expand
- pillow-rs/src/compute/pool_cpu/ops/imageops/geometry.rs: 5: op_contain, 48: op_cover, 93: op_fit, 174: op_crop_border, 242: op_scale
- pillow-rs/src/compute/pool_cpu/ops/imageops/histogram.rs: 38: native_luma_lut_tables, 70: apply_native_luma_lut_row, 90: autocontrast_lut, 383: equalize_lut
- pillow-rs/src/compute/pool_cpu/ops/imageops/pad.rs: 19: op_pad
- pillow-rs/src/compute/pool_cpu/ops/imageops/pad/native.rs: 23: pad_native_rows, 350: pad_native_bytes
- pillow-rs/src/compute/pool_cpu/ops/imageops/pixel.rs: 53: op_autocontrast, 141: op_equalize, 146: op_equalize_with_mask, 173: op_invert, 290: op_flip, 347: op_mirror, 390: op_posterize, 435: op_solarize
- pillow-rs/src/compute/pool_cpu/ops/paste.rs: 667: op_paste
- pillow-rs/src/compute/pool_cpu/ops/paste/kernels.rs: 11: paste_native_rows, 85: paste_rgb_into_rgba_rows, 196: paste_native_masked_l_row, 229: paste_native_masked_pa_row, 301: paste_native_masked_two_channel_blend_byte, 321: paste_native_masked_two_channel_l_row, 360: paste_native_masked_cmyk_row, 404: paste_native_masked_hsv_l_row, 444: paste_native_masked_rgb_l_row, 484: paste_native_masked_rgba_l_row
- pillow-rs/src/compute/pool_cpu/ops/paste/layout.rs: 11: paste_native_channels, 38: value, 47: paste_mask_layout, 83: paste_luma16_storage_byte_shift, 102: paste_mask_pixels
- pillow-rs/src/compute/pool_cpu/ops/put.rs: 5: op_put_pixel, 68: op_put_pixel_batch, 107: op_put_data, 194: op_put_alpha, 276: op_put_alpha_data
- pillow-rs/src/compute/pool_cpu/ops/reduce.rs: 226: execute_reduce_rgb, 256: execute_reduce_rgb_with_sparse_rows, 367: execute_reduce_rgb_sparse_2x2_with_rows, 454: execute_reduce_rgb_sparse_2x2, 471: reduce_rgba_premultiplied_block_sums, 503: execute_reduce
- pillow-rs/src/compute/pool_cpu/ops/reduce/kernels/rgb.rs: 105: execute_reduce_rgb_general, 144: reduce_rgb_dense_even_2x2
- pillow-rs/src/compute/pool_cpu/ops/resize.rs: 25: execute_resize, 64: execute_resize_boxed
- pillow-rs/src/compute/pool_cpu/ops/resize/filter.rs: 7: f_kernel_box, 12: f_kernel_triangle, 18: f_kernel_catrom, 36: f_kernel_lanczos, 55: f_kernel_hamming, 76: f_kernel_lanczos3, 83: resample_kernel
- pillow-rs/src/compute/pool_cpu/ops/resize/float.rs: 32: f_resize_mul_add, 45: f_resize_accumulate, 65: f_resize_horizontal_eight_tap_sample, 116: f_resize_horizontal_sample, 159: f_resize_contiguous_eight_tap_coefficients, 180: f_resize_vertical_sample, 267: f_resize_horizontal_pass, 349: f_resize_vertical_pass, 411: resize_f_tall_order, 527: resize_f, 540: resize_f_with_fma
- pillow-rs/src/compute/pool_cpu/ops/resize/float_resize.rs: 15: accumulate_row, 39: vertical_pass_packed, 65: packed_vertical_output, 114: nearest
- pillow-rs/src/compute/pool_cpu/ops/resize/integer.rs: 20: resize_i_mul_add, 30: resize_i_sum_eight, 48: resize_i_sum_eight_contiguous, 69: resize_i_sum_general, 86: resize_i_round_up_to_i32, 93: resize_i_contiguous_eight_tap_weights, 115: resize_i_horizontal_row, 171: resize_i_streaming_vertical_pass, 229: resize_i_streaming_with_fma, 278: resize_i, 441: resize_i_with_fma, 530: i32_sample_bytes_from_native_storage, 548: i32_samples_from_native_storage, 562: resize_i_boxed
- pillow-rs/src/compute/pool_cpu/ops/rotate.rs: 26: rotate_90_non_expand, 85: execute_rotate
- pillow-rs/src/compute/pool_cpu/ops/rotate/affine.rs: 21: affine_nearest_fixed, 403: rotate_arbitrary_generic
- pillow-rs/src/compute/pool_cpu/ops/rotate/affine/coordinates.rs: 2: ceil_div_positive_denominator, 15: fixed_coordinate_in_bounds_span, 39: nearest_positive_step_in_bounds_span, 80: fill_rgb_pixels, 87: fill_rgba_pixels
- pillow-rs/src/compute/pool_cpu/ops/rotate/cubic.rs: 4: rotate_cubic_horizontal_f32, 16: rotate_cubic_horizontal_f64, 32: rotate_cubic_horizontal_i32, 60: rotate_cubic_vertical_f64_fma, 72: rotate_cubic_vertical_f64
- pillow-rs/src/compute/pool_cpu/ops/rotate/scalar.rs: 17: rotate_arbitrary_scalar
- pillow-rs/src/compute/pool_cpu/ops/thumbnail.rs: 40: execute_thumbnail, 241: rgb_putpixel_thumbnail_fusion_supported, 292: reduce_f_thumbnail_2x2_samples, 327: reduce_f_thumbnail, 463: resize_i_thumbnail_reduce2x2, 481: resize_i_thumbnail_reduce2x2_with_fma, 579: reduce_i_thumbnail
- pillow-rs/src/compute/pool_cpu/ops/thumbnail/fusion.rs: 13: execute_rgb_putpixel_thumbnail_fusion
- pillow-rs/src/compute/pool_cpu/ops/transform.rs: 59: op_transform
- pillow-rs/src/compute/pool_cpu/ops/transform/affine.rs: 9: transform_affine_luma_nearest, 61: transform_affine_luma_alpha_nearest, 117: transform_affine_rgb_nearest, 361: transform_affine_generic
- pillow-rs/src/compute/pool_cpu/ops/transform/projective.rs: 177: transform_projective_generic, 250: transform_luma16, 349: transform_mesh, 494: transform_mesh_with_filter, 602: cubic_sample
- pillow-rs/src/compute/pool_cpu/ops/transform/scalar.rs: 155: transform_affine_f32, 227: sample_transform_f32, 396: f32_sub_c, 420: f32_add_c, 444: f32_fma_c, 475: f64_from_f32_c, 493: f32_from_f64_c, 511: f64_neg_c, 531: f64_sub_c, 555: f64_add_c, 579: f64_fma_c, 616: affine_nearest_fixed
- pillow-rs/src/compute/pool_cpu/ops/transform/scalar/projective.rs: 2: transform_projective_scalar32, 206: transform_mesh_scalar32
- pillow-rs/src/compute/pool_cpu/ops/transpose.rs: 25: should_tile_transpose, 37: transpose_bytes_tiled, 116: transpose_bytes_serial, 153: transpose_bytes_rows, 202: execute_transpose
- pillow-rs/src/compute/pool_cpu/ops/transpose/arch.rs: 7: try_rotate270_rgba_tiled
- pillow-rs/src/compute/pool_cpu/ops/transpose/kernels/transpose_rgba/x86_64.rs: 26: rotate270_tiled
- pillow-rs/src/compute/pool_cpu/ops/transpose/native.rs: 19: execute_flip, 62: execute_axis_swap

### SIMD (569 declarations across 150 files)

- pillow-rs/src/compute/pool_simd/batch_execution.rs: 7: execute_batch
- pillow-rs/src/compute/pool_simd/mod.rs: 38: compose_transpose_methods, 53: transpose_identity_can_reuse
- pillow-rs/src/compute/pool_simd/ops/adapters.rs: 191: simd_unsupported, 197: preserves_native_contract, 255: simd_in_place_supported, 298: simd_execute_in_place
- pillow-rs/src/compute/pool_simd/ops/adapters/alpha_composite.rs: 19: simd_alpha_composite_operands_supported, 47: simd_alpha_composite_shape_supported, 61: supports_for_image, 68: supports_for_shape, 455: simd_alpha_composite_mirror, 558: simd_alpha_composite
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur.rs: 28: simd_native_blur_channels, 33: supports_for_image, 63: supports_for_shape, 117: simd_pil_box_blur, 136: simd_pil_box_blur_xy, 606: simd_box_blur
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur/kernels.rs: 388: simd_blur_rows, 482: simd_transpose_interleaved_rows, 539: simd_blur_transpose_rows
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur/kernels/la.rs: 8: simd_la_blur_scalar_pixel, 38: simd_la_vertical_radius_one_rows
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur/kernels/luma.rs: 129: simd_luma_horizontal_radius_one_three_passes_rows, 176: simd_luma_vertical_radius_one_rows, 247: simd_luma_box_blur_radius_one_rows
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur/kernels/rgb.rs: 140: simd_rgb_horizontal_radius_one_rows, 185: simd_rgb_vertical_radius_one_rows, 326: simd_rgb_box_blur_radius_one_rows
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur/kernels/rgba.rs: 47: simd_rgba_horizontal_radius_one_rows, 114: simd_rgba_vertical_radius_one_rows
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur/kernels/rgba_avx2.rs: 15: blur_middle_block
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur/kernels/rgba_neon.rs: 18: blur_middle_block, 96: blur_vertical_rows_radius_seven
- pillow-rs/src/compute/pool_simd/ops/adapters/chops.rs: 28: materialize_chops_operand, 38: supports_constant_for_image, 42: supports_constant_for_shape, 158: supports_for_image, 211: supports_for_shape, 273: supports_in_place, 295: execute_in_place, 466: simd_multiply, 479: simd_screen, 492: simd_overlay, 503: simd_hard_light, 514: simd_soft_light, 538: simd_add, 573: simd_subtract, 607: simd_fused_multiply_screen, 683: simd_constant
- pillow-rs/src/compute/pool_simd/ops/adapters/chops/kernels.rs: 100: apply_native_blend_rows_in_place, 157: native_chops_blend, 271: native_chops_bytewise_in_place, 396: native_chops_overlay, 404: native_chops_hard_light, 443: native_chops_soft_light, 531: native_chops_pair_channels, 548: native_chops_affine, 623: native_chops_affine_in_place, 744: native_logical_and_vector, 756: native_logical_or_vector, 768: native_logical_xor_vector
- pillow-rs/src/compute/pool_simd/ops/adapters/chops/kernels/fused_multiply_screen.rs: 7: simd_fused_multiply_screen_row
- pillow-rs/src/compute/pool_simd/ops/adapters/color3dlut.rs: 54: color3dlut_supported_for_image, 83: color3dlut_supported_for_shape, 322: simd_color3dlut
- pillow-rs/src/compute/pool_simd/ops/adapters/colorize.rs: 10: supports_for_image, 20: supports_for_shape, 141: simd_colorize
- pillow-rs/src/compute/pool_simd/ops/adapters/convert.rs: 49: supports_lab_for_image, 57: supports_lab_for_shape, 235: native_convert_pixels_per_vector, 262: native_convert_supported_for_image, 343: native_convert_supported_for_shape, 542: simd_convert, 619: simd_convert_lab
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/alpha.rs: 33: supports_image, 60: supports_shape, 89: try_convert
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/cmyk.rs: 13: native_cmyk_to_rgb_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/color_space.rs: 23: native_convert_cmyk_bytes, 109: native_convert_ycbcr_bytes, 190: native_convert_hsv_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/dispatch.rs: 15: native_convert_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/dispatch/layout.rs: 6: append_layout_pixels
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/dispatch/luma.rs: 6: append_rgb_luma_pixels
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/dispatch/typed.rs: 6: append_integer_pixels, 57: append_float_pixels
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/layout.rs: 14: native_convert_layout_block, 142: native_luma_to_rgb_bytes, 179: native_luma_to_rgba_bytes, 232: native_luma_alpha_to_rgba_bytes, 268: native_rgba_to_la_bytes, 318: native_rgba_color_degenerate, 396: native_rgb_to_rgba_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/mod.rs: 27: native_rgba_to_rgb_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/rgba_rgb_neon.rs: 10: native_rgba_to_rgb_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/rgba_rgb_ssse3.rs: 9: native_rgba_to_rgb_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/crop.rs: 12: supports_for_image, 40: supports_for_shape, 234: simd_crop_border, 270: simd_crop
- pillow-rs/src/compute/pool_simd/ops/adapters/draw.rs: 24: native_draw_layout, 91: shape_draw_channels, 114: supports_for_image, 187: supports_for_shape, 261: preserves_native_contract, 282: simd_draw_line, 314: simd_draw_point, 340: simd_draw_rectangle, 401: simd_draw_polygon, 427: simd_draw_rounded_rect, 463: simd_draw_ellipse, 497: simd_draw_circle, 530: simd_draw_arc, 566: simd_draw_chord, 604: simd_draw_pieslice
- pillow-rs/src/compute/pool_simd/ops/adapters/draw/kernels/shapes.rs: 66: simd_draw_ellipse_native, 133: simd_draw_arc_native, 202: simd_draw_chord_native, 288: simd_draw_pieslice_native, 433: simd_draw_polygon_native
- pillow-rs/src/compute/pool_simd/ops/adapters/draw/kernels/shapes/rounded_rect.rs: 223: simd_draw_rounded_rect_native
- pillow-rs/src/compute/pool_simd/ops/adapters/draw/kernels/spans.rs: 69: draw_line_vector_block, 112: simd_draw_span, 286: simd_draw_points_native, 398: simd_draw_rectangle_native
- pillow-rs/src/compute/pool_simd/ops/adapters/draw/kernels/spans/line.rs: 11: simd_draw_line_native
- pillow-rs/src/compute/pool_simd/ops/adapters/duplicate.rs: 15: supports_for_image, 19: supports_for_shape, 45: simd_copy_native_bytes, 71: simd_duplicate
- pillow-rs/src/compute/pool_simd/ops/adapters/effects.rs: 135: native_invert, 206: simd_invert, 214: simd_invert_chops, 223: output_layout, 228: output_mode, 232: supports_for_image, 252: supports_for_shape, 277: supports_in_place, 281: execute_invert_in_place, 294: simd_effect_noise, 489: simd_effect_spread
- pillow-rs/src/compute/pool_simd/ops/adapters/enhance.rs: 23: supports_for_image, 47: supports_for_shape, 76: supports_in_place, 87: execute_in_place, 393: simd_brightness, 450: simd_contrast, 485: simd_color_saturation, 528: execute_brightness_in_place
- pillow-rs/src/compute/pool_simd/ops/adapters/enhance/point.rs: 11: supports_for_image, 18: supports_for_shape, 30: supports_in_place, 106: execute_in_place, 131: simd_solarize, 149: simd_posterize
- pillow-rs/src/compute/pool_simd/ops/adapters/enhance/sharpness.rs: 347: simd_sharpness
- pillow-rs/src/compute/pool_simd/ops/adapters/expand.rs: 11: supports_for_image, 20: supports_for_shape, 279: simd_expand
- pillow-rs/src/compute/pool_simd/ops/adapters/extract_band.rs: 44: supports_for_image, 62: supports_for_shape, 101: simd_extract_band, 160: simd_extract_band_into
- pillow-rs/src/compute/pool_simd/ops/adapters/filter.rs: 21: supports_for_image, 43: supports_for_shape, 80: use_native_byte_convolution_path, 90: use_native_i32_convolution_path, 109: native_filter_identity_supported_for_image, 133: shape_native_filter_byte_channels, 150: shape_native_i32_convolution_path, 161: shape_native_filter_identity_supported, 322: simd_filter_3x3, 470: uniform_rgb_5x5_identity, 506: rgb_blur_ring_5x5, 670: simd_filter_5x5
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/byte_layouts.rs: 112: native_filter_5x5_rows
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/i32_three_by_three.rs: 73: uniform_i32_filter3x3_is_identity_simd, 131: native_filter_3x3_i32_rows
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/l.rs: 9: native_filter_5x5_uniform_luma_rows, 95: native_filter_5x5_binomial_luma_rows
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/la.rs: 54: native_filter_5x5_single_outlier_la_rows
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/mod.rs: 29: scan_uniform_rgb, 39: apply_rgb_blur_ring
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/rgb_blur_ring.rs: 114: apply
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/three_by_three.rs: 16: native_filter_3x3_vector, 69: native_filter_3x3_pixel_block_integer, 118: native_filter_3x3_channel_block_integer, 167: native_filter_3x3_luma_block_integer, 214: native_filter_3x3_rgb_byte_block_integer, 264: native_filter_3x3_pixel_block_vector, 312: native_filter_3x3_la_binomial_rows, 392: native_filter_3x3_la_binomial_rows, 481: native_filter_3x3_rows, 570: native_filter_3x3_rgb_byte_stream
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/uniform_rgb.rs: 19: scan_uniform_rgb
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/unsharp.rs: 13: simd_unsharp_blend_rgb_150
- pillow-rs/src/compute/pool_simd/ops/adapters/flip.rs: 14: supports_for_image, 32: supports_for_shape, 275: simd_flip, 298: simd_mirror
- pillow-rs/src/compute/pool_simd/ops/adapters/gaussian_blur.rs: 16: gaussian_blur_radius, 34: supports_for_image, 45: supports_for_shape, 64: simd_la_horizontal_radius_one_three_passes_rows, 185: simd_pil_gaussian_blur_l_radius_one, 303: simd_gaussian_blur
- pillow-rs/src/compute/pool_simd/ops/adapters/gaussian_blur/kernels/la.rs: 10: horizontal_radius_one
- pillow-rs/src/compute/pool_simd/ops/adapters/gaussian_blur/kernels/rgba_uniform.rs: 21: prove_identity
- pillow-rs/src/compute/pool_simd/ops/adapters/gradient.rs: 14: simd_linear_gradient_generate, 91: simd_radial_gradient_generate
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale.rs: 52: supports_image, 56: supports_shape, 83: simd_grayscale_with_evidence, 129: simd_grayscale
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/kernels/channels.rs: 180: native_grayscale_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/kernels/cmyk.rs: 147: native_cmyk_grayscale_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/kernels/rgb_avx2.rs: 10: grayscale_rgb_avx2
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/kernels/ycbcr.rs: 10: native_ycbcr_grayscale_bytes, 78: native_ycbcr_grayscale_path
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/kernels/ycbcr_ssse3.rs: 10: ycbcr_luma_ssse3_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/histogram.rs: 42: supports_for_image, 64: supports_for_shape, 97: simd_autocontrast, 144: simd_equalize
- pillow-rs/src/compute/pool_simd/ops/adapters/layout.rs: 9: native_byte_layout, 28: native_filter_byte_layout, 44: native_typed_filter_layout, 67: native_expand_contract_for_image, 88: native_expand_contract_for_shape, 117: native_copy_layout, 140: native_luma16_transpose_layout, 154: has_valid_luma16_copy_bytes, 171: has_vectorized_byte_rows, 183: has_empty_native_bytes, 197: has_vectorized_offset_rows, 220: has_vectorized_luma16_offset, 237: has_vectorized_mirror_rows, 245: has_nonempty_byte_data, 259: has_valid_byte_data, 273: has_vectorized_native_identity_copy, 303: simd_shape, 319: shape_channels, 345: shape_native_byte_channels, 361: shape_native_typed_filter_channels, 368: shape_has_nonempty_byte_data, 378: shape_has_vectorized_native_identity_copy, 389: shape_native_copy_channels, 397: shape_luma16_transpose_layout, 402: shape_luma16_copy_layout, 406: shape_native_identity_copy_channels, 421: shape_has_empty_native_bytes, 430: shape_has_vector_rows, 438: shape_has_vectorized_offset_rows, 455: shape_has_vectorized_luma16_offset, 464: shape_has_vectorized_mirror_rows, 688: shape_after_simd_op
- pillow-rs/src/compute/pool_simd/ops/adapters/lut.rs: 38: supports_for_image, 52: supports_for_shape, 77: in_place_supported, 87: execute_in_place, 103: native_lut_tables, 116: native_lut_chunk, 220: native_lut_tables_for_channels, 261: native_lut_apply, 360: native_lut_map_rows, 622: native_lut_is_identity, 658: native_point_lut, 734: simd_eval, 756: simd_remap_palette
- pillow-rs/src/compute/pool_simd/ops/adapters/mandelbrot.rs: 12: simd_mandelbrot_generate
- pillow-rs/src/compute/pool_simd/ops/adapters/median_filter.rs: 21: supports_for_image, 70: supports_for_shape, 123: native_float_rank_supported_for_image, 145: shape_native_float_rank_supported, 212: median_of_nine_vectors, 218: order_statistic_row, 553: simd_order_statistic_filter
- pillow-rs/src/compute/pool_simd/ops/adapters/median_filter/extrema.rs: 438: simd_max_filter, 458: simd_min_filter
- pillow-rs/src/compute/pool_simd/ops/adapters/median_filter/float.rs: 178: simd_float_order_statistic_filter, 257: simd_median_filter, 272: simd_rank_filter
- pillow-rs/src/compute/pool_simd/ops/adapters/median_filter/kernels.rs: 18: median_of_nine_vectors, 49: median_filter_rgb_3x3_row, 94: la_order_statistic_3x3_row
- pillow-rs/src/compute/pool_simd/ops/adapters/merge.rs: 110: supports_image_contract, 120: supports_shape_contract, 130: native_merge_interleave, 196: native_merge_rgb_luma_owners, 236: native_merge_bytes, 291: simd_merge
- pillow-rs/src/compute/pool_simd/ops/adapters/module_blend.rs: 12: supports_for_image, 21: supports_for_shape, 172: native_module_blend_arithmetic_supported, 293: simd_blend_module
- pillow-rs/src/compute/pool_simd/ops/adapters/native_arithmetic.rs: 5: simd_div255, 16: simd_div255_u16x8, 23: simd_pack_u16x16
- pillow-rs/src/compute/pool_simd/ops/adapters/native_bytes.rs: 34: native_expand_fill_sample, 63: native_fill_row, 153: native_byte_transform_bytes, 187: native_all_channel_transform_bytes, 211: native_byte_transform_copy_bytes, 233: apply_native_rows, 272: reverse_pixel_block, 328: copy_native_bytes, 360: native_copy_image_bytes, 394: mirror_native_row, 456: native_luma16_image_from_bytes, 476: record_padded_native_row_work
- pillow-rs/src/compute/pool_simd/ops/adapters/neighborhood.rs: 16: native_zero_byte_image, 29: native_small_uniform_byte_image, 38: native_uniform_byte_image_up_to, 58: simd_blur_load_u8x16, 63: simd_blur_pack_u32x16_to_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/offset.rs: 20: supports_for_image, 28: supports_for_shape, 36: simd_offset
- pillow-rs/src/compute/pool_simd/ops/adapters/offset/kernels/byte.rs: 13: circular_byte_block, 74: narrow_rotated_row, 189: native_offset
- pillow-rs/src/compute/pool_simd/ops/adapters/offset/kernels/luma16.rs: 165: native_offset_luma16
- pillow-rs/src/compute/pool_simd/ops/adapters/pad.rs: 173: supports_for_image, 180: supports_for_shape, 651: simd_pad
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/apply.rs: 54: native_paste_apply, 75: native_paste_apply_with_source_layout
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/apply/cmyk_l.rs: 6: apply
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/apply/generic.rs: 151: apply
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/apply/l.rs: 8: apply
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/apply/la.rs: 8: apply
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/apply/pa.rs: 8: apply
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/apply/rgb.rs: 28: apply
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/apply/rgba.rs: 6: apply
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/blend.rs: 11: native_paste_mask_block, 36: native_paste_mask_block_active, 70: native_paste_blend_vector16, 85: native_paste_binary_copy_vector16, 97: native_paste_blend_vector8, 114: native_paste_preblend_vector16, 131: native_paste_preblend_vector8, 148: native_paste_rgb_l_mask_block16, 171: native_paste_rgb_l_mask_block8
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/luma.rs: 9: native_paste_l_masked_row, 107: native_paste_rgba_l_masked_row, 423: native_paste_la_masked_row
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/luma16.rs: 200: native_paste_luma16_apply
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/rgb_cmyk.rs: 12: native_paste_pa_masked_row, 185: native_paste_rgb_l_masked_row, 345: native_paste_cmyk_l_masked_row
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/mod.rs: 46: simd_composite_module, 205: simd_paste, 304: native_paste_in_place
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/plan.rs: 66: native_paste_layout, 150: native_paste_shape_layout, 272: native_paste_region, 401: native_paste_plan_for_image, 418: native_paste_plan_for_image_with_source_layout, 446: native_paste_plan_for_shape, 479: native_paste_rgba_to_rgb_supported, 500: native_paste_rgba_to_rgb_supported_for_shape, 524: native_paste_rgb_to_rgba_supported, 545: native_paste_rgb_to_rgba_supported_for_shape, 575: native_composite_plan_for_image, 620: native_composite_plan_for_shape, 663: native_paste_actual_layout, 683: native_paste_actual_mask_layout
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/rgb_to_rgba.rs: 119: simd_paste_rgb_to_rgba
- pillow-rs/src/compute/pool_simd/ops/adapters/pipeline.rs: 26: pixel_mode_name, 80: simd_initial_mode, 108: simd_mode_after_op, 133: first_unsupported_simd_op
- pillow-rs/src/compute/pool_simd/ops/adapters/projection.rs: 16: simd_cmyk_projection, 88: simd_cmyk_projection_from_positive_writes, 164: simd_rgb_projection
- pillow-rs/src/compute/pool_simd/ops/adapters/put_alpha.rs: 20: supports_for_image, 36: supports_for_shape, 69: update_shape, 80: output_mode, 257: simd_put_alpha, 361: simd_put_alpha_data
- pillow-rs/src/compute/pool_simd/ops/adapters/put_alpha/kernels.rs: 153: simd_put_alpha_data_bytes, 221: simd_put_alpha_rgb_row, 251: simd_put_alpha_rgb_mask_row, 284: simd_put_alpha_rgba_mask_row, 293: simd_put_alpha_rgba_constant_bytes, 320: simd_put_alpha_rgb_constant_bytes, 385: simd_put_alpha_constant_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/put_data.rs: 34: supports_for_image, 43: supports_for_shape, 55: simd_put_data
- pillow-rs/src/compute/pool_simd/ops/adapters/put_pixel.rs: 11: simd_rgba_putpixel_batch_supported, 48: simd_rgba_putpixel_batch, 200: native_putpixel_layout, 208: native_putpixel_supported_for_image, 226: native_putpixel_supported_for_shape, 302: simd_put_pixel
- pillow-rs/src/compute/pool_simd/ops/adapters/put_pixel/kernels.rs: 9: apply_rgba_batch
- pillow-rs/src/compute/pool_simd/ops/adapters/put_pixel/kernels/rgba_batch.rs: 8: apply_rgba_batch
- pillow-rs/src/compute/pool_simd/ops/adapters/reduce.rs: 19: native_reduce_layout, 35: shape_native_reduce_layout, 193: native_reduce_supported_for_image, 232: native_reduce_supported_for_shape, 294: native_reduce_sparse_rgb_2x2, 456: native_reduce_bytes, 562: simd_reduce
- pillow-rs/src/compute/pool_simd/ops/adapters/reduce/kernels.rs: 2: native_reduce_pixel_sums, 40: native_reduce_average, 46: native_reduce_scalar_pixel, 114: native_reduce_3byte_2x2_vector_block, 187: native_reduce_rgb_3x5_vector_block, 237: native_reduce_vector_block
- pillow-rs/src/compute/pool_simd/ops/adapters/resize.rs: 90: supports_for_image, 122: supports_for_shape, 158: resize_native_channels_for_image, 171: native_resize_byte_layout_for_image, 187: native_resize_byte_layout_for_shape, 204: simd_resize, 438: simd_resize_impl
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/aspect.rs: 84: simd_scale, 111: simd_contain, 143: simd_cover, 170: simd_fit
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/coordinates.rs: 3: resize_nearest_indices, 22: resize_nearest_vectorizable, 52: boxed_nearest_indices, 82: boxed_nearest_axis_maps_identity
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/f32.rs: 35: simd_resize_f
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/f32/boxed.rs: 6: simd_resize_f_boxed, 325: write_f32_samples_le
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/i32.rs: 50: build_resize_i_horizontal_plan, 150: resize_i_horizontal_values, 168: simd_resize_i32, 537: simd_resize_i32_boxed
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/i32/neon.rs: 32: horizontal_block, 101: vertical_block
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/luma16.rs: 21: simd_resize_luma16
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/u8.rs: 23: simd_resize_convolution, 45: simd_resize_convolution_with_parallel_threshold, 70: simd_resize_convolution_with_zero_row_elision, 106: simd_resize_convolution_into
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/u8_boxed_convolution.rs: 28: simd_resize_convolution_boxed
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/u8_convolution.rs: 10: resize_fixed_point_to_u8, 27: resize_premultiply_u8, 50: resize_unpremultiply_u8, 55: resize_horizontal_scalar, 81: resize_horizontal_vector_row, 177: resize_horizontal_convolution_row, 221: resize_vertical_vector_row
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/u8_cover_i32.rs: 315: resize_horizontal_hsv_i32_vector_row, 396: resize_vertical_hsv_i32_vector_row, 666: resize_horizontal_cover_i32_vector_row, 713: resize_vertical_cover_i32_vector_row
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/u8_nearest.rs: 136: simd_resize_nearest, 202: simd_resize_nearest_boxed
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/u8_plan.rs: 7: resize_coeff_slice, 19: resize_vertical_row_touches_nonzero_source, 43: resize_u8_coefficients_fit_i32, 91: build_resize_horizontal_plan
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/support/aspect.rs: 14: native_pad_round_dimension, 36: native_pad_contained_dimensions, 71: native_cover_dimensions, 111: native_fit_box, 159: native_fit_dimensions_supported, 179: native_fit_layout_for_image, 198: native_fit_layout_for_shape, 213: native_fit_filter, 226: native_resize_boxed_bounds, 253: native_resize_boxed_layout_for_image, 269: native_resize_boxed_layout_for_shape, 285: native_resize_boxed_supported_for_dimensions, 327: native_resize_boxed_supported_for_image, 358: native_resize_boxed_supported_for_shape, 382: native_fit_float_supported_for_image, 416: native_fit_float_supported_for_shape, 437: native_fit_supported_for_image, 505: native_fit_supported_for_shape, 563: native_pad_channels_for_image, 582: native_pad_channels_for_shape, 597: native_contain_supported_for_image, 637: native_contain_supported_for_shape, 665: native_cover_supported_for_image, 697: native_cover_supported_for_shape
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/support/resize.rs: 11: native_scale_dimensions, 25: native_resize_supported_for_dimensions, 76: native_resize_supported_for_image, 131: native_resize_supported_for_shape
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate.rs: 198: rotate_discrete_fast_angle, 213: rotate_fill_sample, 285: supports_for_image, 359: supports_for_shape, 434: copy_nearest_affine_pixels, 512: copy_right_angle_pixels, 597: finish_native_rotation, 614: simd_rotate
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate/interpolation.rs: 8: simd_rotate_sample_coordinates, 39: bilinear_rotate_scalar, 53: rotate_fused_mul_add, 80: bilinear_rotate_vector, 100: gather_rotate_neighborhood_vectors, 161: rotate_unpremultiply_vector
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate/kernels/native.rs: 22: native_identity_rotate, 36: simd_right_angle_rotate_native, 138: simd_empty_rotate_native
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate/kernels/native/bilinear.rs: 157: simd_bilinear_rotate_native
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate/kernels/native/nearest.rs: 27: simd_nearest_rotate_native
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate/kernels/native/typed_bilinear.rs: 121: simd_typed_bilinear_rotate_native
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate/kernels/nearest.rs: 14: copy_contiguous_rgb_runs
- pillow-rs/src/compute/pool_simd/ops/adapters/support.rs: 112: simd_supports_for_image, 330: simd_supports_for_shape
- pillow-rs/src/compute/pool_simd/ops/adapters/thumbnail.rs: 40: native_thumbnail_dimensions, 146: native_thumbnail_supported_for_image, 186: native_thumbnail_supported_for_shape, 232: simd_thumbnail
- pillow-rs/src/compute/pool_simd/ops/adapters/thumbnail/kernels/float.rs: 14: thumbnail_f_reduce_value, 184: simd_thumbnail_reduce_f
- pillow-rs/src/compute/pool_simd/ops/adapters/thumbnail/kernels/fusion.rs: 14: simd_rgb_putpixel_thumbnail_fusion_supported, 126: simd_rgb_putpixel_thumbnail_prepare, 177: simd_rgb_putpixel_thumbnail_resize
- pillow-rs/src/compute/pool_simd/ops/adapters/thumbnail/kernels/integer.rs: 17: simd_thumbnail_reduce_i, 126: simd_thumbnail_i_reduce2x2_resample
- pillow-rs/src/compute/pool_simd/ops/adapters/transform.rs: 255: supports_for_image, 283: supports_for_shape, 314: simd_transform
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/kernels/bilinear/affine.rs: 157: simd_affine_bilinear_transform_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/kernels/bilinear/projective.rs: 106: simd_projective_bilinear_transform_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/kernels/luma16.rs: 9: simd_affine_luma16_transform_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/kernels/mesh.rs: 90: simd_mesh_transform_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/kernels/nearest/affine.rs: 177: simd_affine_nearest_transform_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/kernels/nearest/projective.rs: 63: simd_projective_nearest_transform_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/transpose.rs: 34: supports_for_image, 43: supports_for_shape, 54: rgba_avx2, 286: native_transpose_bytes, 427: native_transpose, 469: simd_transpose
- pillow-rs/src/compute/pool_simd/ops/adapters/transpose/kernels.rs: 22: transpose_rgba
- pillow-rs/src/compute/pool_simd/ops/adapters/transpose/kernels/portable.rs: 35: transpose_gathered_row, 232: transpose_native_tiled, 287: transpose_native_collect_admitted, 319: finish, 350: new, 367: block, 419: transpose_native_collect, 471: transpose_native_odd_collect_admitted, 507: finish, 542: new, 558: block, 613: transpose_native_odd_collect
- pillow-rs/src/compute/pool_simd/ops/adapters/transpose/kernels/rgba_avx2.rs: 14: transpose_rgba
- pillow-rs/src/compute/pool_simd/ops/native_channel.rs: 47: gather_interleaved_channel, 66: gather_channel_into, 95: gather_channel, 115: extract_rgb_channel_bytes, 291: extract_channel_into
- pillow-rs/src/compute/pool_simd/ops/native_channel/cmyk_ssse3.rs: 32: deinterleave_cmyk_16_ssse3
- pillow-rs/src/compute/pool_simd/ops/native_filter.rs: 6: pack_u16x16, 15: native_filter_load_byte_block, 31: native_filter_5x5_binomial_la_rows, 236: native_filter_5x5_i32_rows
- pillow-rs/src/compute/pool_simd/ops/native_merge.rs: 67: rgb16_interleave_ssse3, 100: vector_block, 135: rgb_interleave_ssse3, 235: rgb_interleave_neon
- pillow-rs/src/compute/pool_simd/point_batch.rs: 60: fused
- pillow-rs/src/compute/pool_simd/put_pixel_batch.rs: 9: try_execute

## 9. DynamicImage variant consumers affected by the mode enum

The baseline has 262 Rust files with 2100 explicit references to one of the ten legacy DynamicImage variant names across the tracked repository. This is a source search count, not a count of exhaustive match arms: it includes constructors, accessors, tests and comments. Review every listed file when replacing the enum; files using wildcard or generic methods without an explicit variant are not included in this list. Keep tests' inputs and assertions unchanged while adapting only the new type shape.

### pillow-rs (262 files)

- pillow-rs/src/color/conversion.rs: 26 references
- pillow-rs/src/color/grayscale.rs: 7 references
- pillow-rs/src/color/tests.rs: 8 references
- pillow-rs/src/color_grayscale_allocation_tests.rs: 9 references
- pillow-rs/src/compute/execution.rs: 7 references
- pillow-rs/src/compute/mod.rs: 3 references
- pillow-rs/src/compute/pool_cpu/batch_execution.rs: 1 references
- pillow-rs/src/compute/pool_cpu/mod.rs: 16 references
- pillow-rs/src/compute/pool_cpu/ops/chops.rs: 13 references
- pillow-rs/src/compute/pool_cpu/ops/color.rs: 45 references
- pillow-rs/src/compute/pool_cpu/ops/color3dlut.rs: 2 references
- pillow-rs/src/compute/pool_cpu/ops/color_convert_allocation_tests.rs: 12 references
- pillow-rs/src/compute/pool_cpu/ops/composite.rs: 30 references
- pillow-rs/src/compute/pool_cpu/ops/draw/canvas.rs: 10 references
- pillow-rs/src/compute/pool_cpu/ops/effects.rs: 14 references
- pillow-rs/src/compute/pool_cpu/ops/enhance.rs: 36 references
- pillow-rs/src/compute/pool_cpu/ops/eval.rs: 10 references
- pillow-rs/src/compute/pool_cpu/ops/filter/blur.rs: 9 references
- pillow-rs/src/compute/pool_cpu/ops/filter/blur/tests.rs: 3 references
- pillow-rs/src/compute/pool_cpu/ops/filter/filter3x3.rs: 8 references
- pillow-rs/src/compute/pool_cpu/ops/filter/filter3x3/tests.rs: 1 references
- pillow-rs/src/compute/pool_cpu/ops/filter/filter5x5.rs: 14 references
- pillow-rs/src/compute/pool_cpu/ops/filter/filter5x5/int32.rs: 2 references
- pillow-rs/src/compute/pool_cpu/ops/filter/filter5x5/int32/tests.rs: 5 references
- pillow-rs/src/compute/pool_cpu/ops/filter/rank.rs: 4 references
- pillow-rs/src/compute/pool_cpu/ops/filter/rank/float.rs: 4 references
- pillow-rs/src/compute/pool_cpu/ops/filter/rank/tests.rs: 1 references
- pillow-rs/src/compute/pool_cpu/ops/grayscale.rs: 14 references
- pillow-rs/src/compute/pool_cpu/ops/imageops/colorize.rs: 2 references
- pillow-rs/src/compute/pool_cpu/ops/imageops/expand.rs: 20 references
- pillow-rs/src/compute/pool_cpu/ops/imageops/geometry.rs: 4 references
- pillow-rs/src/compute/pool_cpu/ops/imageops/histogram.rs: 6 references
- pillow-rs/src/compute/pool_cpu/ops/imageops/pad.rs: 12 references
- pillow-rs/src/compute/pool_cpu/ops/imageops/pad/native.rs: 20 references
- pillow-rs/src/compute/pool_cpu/ops/imageops/pad/tests.rs: 13 references
- pillow-rs/src/compute/pool_cpu/ops/imageops/pixel.rs: 43 references
- pillow-rs/src/compute/pool_cpu/ops/paste.rs: 2 references
- pillow-rs/src/compute/pool_cpu/ops/paste/kernels.rs: 4 references
- pillow-rs/src/compute/pool_cpu/ops/paste/layout.rs: 4 references
- pillow-rs/src/compute/pool_cpu/ops/paste/tests.rs: 18 references
- pillow-rs/src/compute/pool_cpu/ops/put.rs: 48 references
- pillow-rs/src/compute/pool_cpu/ops/reduce.rs: 4 references
- pillow-rs/src/compute/pool_cpu/ops/reduce/tests.rs: 6 references
- pillow-rs/src/compute/pool_cpu/ops/resize.rs: 5 references
- pillow-rs/src/compute/pool_cpu/ops/resize/float.rs: 6 references
- pillow-rs/src/compute/pool_cpu/ops/resize/float_resize.rs: 2 references
- pillow-rs/src/compute/pool_cpu/ops/resize/integer.rs: 5 references
- pillow-rs/src/compute/pool_cpu/ops/resize/tests.rs: 14 references
- pillow-rs/src/compute/pool_cpu/ops/rotate/affine.rs: 3 references
- pillow-rs/src/compute/pool_cpu/ops/rotate/tests.rs: 4 references
- pillow-rs/src/compute/pool_cpu/ops/thumbnail.rs: 12 references
- pillow-rs/src/compute/pool_cpu/ops/thumbnail/fusion.rs: 3 references
- pillow-rs/src/compute/pool_cpu/ops/thumbnail/tests.rs: 14 references
- pillow-rs/src/compute/pool_cpu/ops/transform.rs: 1 references
- pillow-rs/src/compute/pool_cpu/ops/transform/affine.rs: 10 references
- pillow-rs/src/compute/pool_cpu/ops/transform/projective.rs: 8 references
- pillow-rs/src/compute/pool_cpu/ops/transform/scalar.rs: 2 references
- pillow-rs/src/compute/pool_cpu/ops/transform/scalar/projective.rs: 4 references
- pillow-rs/src/compute/pool_cpu/ops/transform/tests.rs: 13 references
- pillow-rs/src/compute/pool_cpu/ops/transpose/tests.rs: 5 references
- pillow-rs/src/compute/pool_gpu/auxiliary_images.rs: 9 references
- pillow-rs/src/compute/pool_gpu/batch_auxiliary_ranges.rs: 1 references
- pillow-rs/src/compute/pool_gpu/batch_execution/routing/execution.rs: 1 references
- pillow-rs/src/compute/pool_gpu/batch_execution/routing/execution/standard.rs: 2 references
- pillow-rs/src/compute/pool_gpu/batch_native_routes.rs: 5 references
- pillow-rs/src/compute/pool_gpu/batch_normalization.rs: 13 references
- pillow-rs/src/compute/pool_gpu/batch_preparation.rs: 1 references
- pillow-rs/src/compute/pool_gpu/batch_telemetry.rs: 2 references
- pillow-rs/src/compute/pool_gpu/batch_upload.rs: 5 references
- pillow-rs/src/compute/pool_gpu/batch_upload/packing.rs: 2 references
- pillow-rs/src/compute/pool_gpu/geometry/native_execution/flip.rs: 2 references
- pillow-rs/src/compute/pool_gpu/geometry/native_execution/transpose.rs: 5 references
- pillow-rs/src/compute/pool_gpu/layout_admission.rs: 9 references
- pillow-rs/src/compute/pool_gpu/logical_mode_admission.rs: 6 references
- pillow-rs/src/compute/pool_gpu/mode.rs: 10 references
- pillow-rs/src/compute/pool_gpu/native_dispatch.rs: 8 references
- pillow-rs/src/compute/pool_gpu/native_hsv_pad.rs: 2 references
- pillow-rs/src/compute/pool_gpu/native_input.rs: 21 references
- pillow-rs/src/compute/pool_gpu/native_input/channels.rs: 13 references
- pillow-rs/src/compute/pool_gpu/native_paste.rs: 35 references
- pillow-rs/src/compute/pool_gpu/native_paste/execution.rs: 7 references
- pillow-rs/src/compute/pool_gpu/native_paste/execution/rgb_to_rgba.rs: 3 references
- pillow-rs/src/compute/pool_gpu/native_paste/masked_execution.rs: 12 references
- pillow-rs/src/compute/pool_gpu/operation_admission.rs: 1 references
- pillow-rs/src/compute/pool_gpu/operation_admission/auxiliary_modes.rs: 10 references
- pillow-rs/src/compute/pool_gpu/operation_support.rs: 4 references
- pillow-rs/src/compute/pool_gpu/point_ops.rs: 40 references
- pillow-rs/src/compute/pool_gpu/point_ops/native_alpha.rs: 5 references
- pillow-rs/src/compute/pool_gpu/projection.rs: 4 references
- pillow-rs/src/compute/pool_gpu/readback.rs: 7 references
- pillow-rs/src/compute/pool_gpu/resize/compact_box.rs: 6 references
- pillow-rs/src/compute/pool_gpu/resize/float_exactness.rs: 4 references
- pillow-rs/src/compute/pool_gpu/resize/float_exactness/word_paths.rs: 1 references
- pillow-rs/src/compute/pool_gpu/resize/float_exactness_f64.rs: 2 references
- pillow-rs/src/compute/pool_gpu/resize/float_fast_paths.rs: 10 references
- pillow-rs/src/compute/pool_gpu/resize/integer.rs: 2 references
- pillow-rs/src/compute/pool_gpu/resize/luma16.rs: 1 references
- pillow-rs/src/compute/pool_gpu/resource_pool.rs: 2 references
- pillow-rs/src/compute/pool_gpu/rgb_merge.rs: 1 references
- pillow-rs/src/compute/pool_gpu/rgb_merge/execution.rs: 5 references
- pillow-rs/src/compute/pool_gpu/stream/encoding.rs: 4 references
- pillow-rs/src/compute/pool_gpu/tests/affine_geometry.rs: 5 references
- pillow-rs/src/compute/pool_gpu/tests/mod.rs: 4 references
- pillow-rs/src/compute/pool_gpu/tests/native_cover.rs: 6 references
- pillow-rs/src/compute/pool_gpu/tests/native_effects.rs: 24 references
- pillow-rs/src/compute/pool_gpu/tests/native_filters.rs: 29 references
- pillow-rs/src/compute/pool_gpu/tests/native_luma/mode_admission.rs: 18 references
- pillow-rs/src/compute/pool_gpu/tests/native_luma/native_byte_filter.rs: 5 references
- pillow-rs/src/compute/pool_gpu/tests/native_luma/order_statistic.rs: 3 references
- pillow-rs/src/compute/pool_gpu/tests/native_luma_transfer.rs: 2 references
- pillow-rs/src/compute/pool_gpu/tests/native_padding.rs: 11 references
- pillow-rs/src/compute/pool_gpu/tests/native_paste_execution.rs: 3 references
- pillow-rs/src/compute/pool_gpu/tests/native_paste_planning/masked_paste_execution.rs: 6 references
- pillow-rs/src/compute/pool_gpu/tests/native_paste_planning/packing_and_native_planners.rs: 6 references
- pillow-rs/src/compute/pool_gpu/tests/palette_projective/palette_alpha_proofs.rs: 1 references
- pillow-rs/src/compute/pool_gpu/tests/pipeline_fusion.rs: 5 references
- pillow-rs/src/compute/pool_gpu/tests/point_merge.rs: 11 references
- pillow-rs/src/compute/pool_gpu/tests/projective_filtered.rs: 1 references
- pillow-rs/src/compute/pool_gpu/tests/projective_nearest_execution.rs: 4 references
- pillow-rs/src/compute/pool_gpu/tests/projective_nearest_proofs.rs: 3 references
- pillow-rs/src/compute/pool_gpu/tests/readback.rs: 2 references
- pillow-rs/src/compute/pool_gpu/tests/resize_float_proofs/box_and_average_proofs.rs: 13 references
- pillow-rs/src/compute/pool_gpu/tests/resize_float_proofs/dyadic_and_f64_proofs.rs: 10 references
- pillow-rs/src/compute/pool_gpu/tests/resize_float_proofs/finite_words_and_integer_rounding.rs: 3 references
- pillow-rs/src/compute/pool_gpu/tests/resize_integer.rs: 3 references
- pillow-rs/src/compute/pool_gpu/tests/resize_thumbnail.rs: 1 references
- pillow-rs/src/compute/pool_gpu/tests/resource_pool.rs: 1 references
- pillow-rs/src/compute/pool_gpu/tests/rgb_compact.rs: 5 references
- pillow-rs/src/compute/pool_gpu/tests/transpose_execution.rs: 5 references
- pillow-rs/src/compute/pool_gpu/tests/transpose_planning.rs: 7 references
- pillow-rs/src/compute/pool_gpu/tests/typed_resize.rs: 1 references
- pillow-rs/src/compute/pool_gpu/tests/upload_reuse.rs: 1 references
- pillow-rs/src/compute/pool_gpu/thumbnail.rs: 2 references
- pillow-rs/src/compute/pool_gpu/transform/geometry.rs: 6 references
- pillow-rs/src/compute/pool_gpu/transform/projective_exactness.rs: 4 references
- pillow-rs/src/compute/pool_gpu/transform/safety.rs: 2 references
- pillow-rs/src/compute/pool_simd/batch_execution.rs: 1 references
- pillow-rs/src/compute/pool_simd/mod.rs: 14 references
- pillow-rs/src/compute/pool_simd/ops/adapters/alpha_composite.rs: 8 references
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur.rs: 6 references
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur/tests.rs: 2 references
- pillow-rs/src/compute/pool_simd/ops/adapters/chops.rs: 12 references
- pillow-rs/src/compute/pool_simd/ops/adapters/color3dlut.rs: 4 references
- pillow-rs/src/compute/pool_simd/ops/adapters/colorize.rs: 3 references
- pillow-rs/src/compute/pool_simd/ops/adapters/convert.rs: 12 references
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/alpha.rs: 4 references
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/cmyk.rs: 1 references
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/layout.rs: 6 references
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/rgba_rgb_neon.rs: 2 references
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/rgba_rgb_ssse3.rs: 2 references
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/tests.rs: 8 references
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/tests/cmyk_packing.rs: 3 references
- pillow-rs/src/compute/pool_simd/ops/adapters/draw.rs: 4 references
- pillow-rs/src/compute/pool_simd/ops/adapters/draw/tests.rs: 2 references
- pillow-rs/src/compute/pool_simd/ops/adapters/effects.rs: 9 references
- pillow-rs/src/compute/pool_simd/ops/adapters/effects/tests.rs: 2 references
- pillow-rs/src/compute/pool_simd/ops/adapters/enhance.rs: 30 references
- pillow-rs/src/compute/pool_simd/ops/adapters/enhance/point.rs: 10 references
- pillow-rs/src/compute/pool_simd/ops/adapters/enhance/tests.rs: 11 references
- pillow-rs/src/compute/pool_simd/ops/adapters/expand/tests.rs: 2 references
- pillow-rs/src/compute/pool_simd/ops/adapters/extract_band.rs: 13 references
- pillow-rs/src/compute/pool_simd/ops/adapters/filter.rs: 8 references
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/l.rs: 4 references
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/la.rs: 1 references
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/tests.rs: 4 references
- pillow-rs/src/compute/pool_simd/ops/adapters/flip.rs: 24 references
- pillow-rs/src/compute/pool_simd/ops/adapters/gaussian_blur.rs: 3 references
- pillow-rs/src/compute/pool_simd/ops/adapters/gaussian_blur/tests.rs: 1 references
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale.rs: 4 references
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/kernels/cmyk.rs: 1 references
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/kernels/ycbcr.rs: 2 references
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/tests.rs: 5 references
- pillow-rs/src/compute/pool_simd/ops/adapters/histogram.rs: 2 references
- pillow-rs/src/compute/pool_simd/ops/adapters/layout.rs: 25 references
- pillow-rs/src/compute/pool_simd/ops/adapters/lut.rs: 5 references
- pillow-rs/src/compute/pool_simd/ops/adapters/lut/tests.rs: 1 references
- pillow-rs/src/compute/pool_simd/ops/adapters/median_filter.rs: 1 references
- pillow-rs/src/compute/pool_simd/ops/adapters/median_filter/tests.rs: 2 references
- pillow-rs/src/compute/pool_simd/ops/adapters/merge.rs: 6 references
- pillow-rs/src/compute/pool_simd/ops/adapters/merge/tests.rs: 1 references
- pillow-rs/src/compute/pool_simd/ops/adapters/module_blend.rs: 8 references
- pillow-rs/src/compute/pool_simd/ops/adapters/native_bytes.rs: 1 references
- pillow-rs/src/compute/pool_simd/ops/adapters/offset/kernels/luma16.rs: 2 references
- pillow-rs/src/compute/pool_simd/ops/adapters/pad.rs: 2 references
- pillow-rs/src/compute/pool_simd/ops/adapters/pad/tests.rs: 4 references
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/mod.rs: 7 references
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/plan.rs: 21 references
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/rgb_to_rgba.rs: 4 references
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/tests.rs: 16 references
- pillow-rs/src/compute/pool_simd/ops/adapters/pipeline.rs: 4 references
- pillow-rs/src/compute/pool_simd/ops/adapters/put_alpha.rs: 18 references
- pillow-rs/src/compute/pool_simd/ops/adapters/put_pixel.rs: 7 references
- pillow-rs/src/compute/pool_simd/ops/adapters/reduce.rs: 6 references
- pillow-rs/src/compute/pool_simd/ops/adapters/reduce/tests.rs: 2 references
- pillow-rs/src/compute/pool_simd/ops/adapters/resize.rs: 3 references
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/aspect.rs: 5 references
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/i32/neon.rs: 1 references
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/luma16.rs: 2 references
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/support/aspect.rs: 12 references
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/support/resize.rs: 2 references
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/tests.rs: 10 references
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate.rs: 6 references
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate/kernels/native.rs: 1 references
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate/kernels/native/typed_bilinear.rs: 1 references
- pillow-rs/src/compute/pool_simd/ops/adapters/thumbnail.rs: 11 references
- pillow-rs/src/compute/pool_simd/ops/adapters/thumbnail/kernels/float.rs: 1 references
- pillow-rs/src/compute/pool_simd/ops/adapters/thumbnail/kernels/fusion.rs: 4 references
- pillow-rs/src/compute/pool_simd/ops/adapters/thumbnail/kernels/integer.rs: 2 references
- pillow-rs/src/compute/pool_simd/ops/adapters/transform.rs: 6 references
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/kernels/luma16.rs: 4 references
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/tests.rs: 2 references
- pillow-rs/src/compute/pool_simd/ops/adapters/transpose.rs: 6 references
- pillow-rs/src/compute/pool_simd/point_batch.rs: 4 references
- pillow-rs/src/compute/transpose_tests.rs: 1 references
- pillow-rs/src/draw/bitmap.rs: 17 references
- pillow-rs/src/draw/mod.rs: 2 references
- pillow-rs/src/draw/text/composition.rs: 10 references
- pillow-rs/src/font/pilfont.rs: 1 references
- pillow-rs/src/image/backend.rs: 5 references
- pillow-rs/src/image/channel_access.rs: 2 references
- pillow-rs/src/image/color_count/tests.rs: 1 references
- pillow-rs/src/image/construction.rs: 22 references
- pillow-rs/src/image/data_access/get.rs: 11 references
- pillow-rs/src/image/entropy.rs: 5 references
- pillow-rs/src/image/materialization.rs: 6 references
- pillow-rs/src/image/palette.rs: 5 references
- pillow-rs/src/image/pipeline_admission.rs: 1 references
- pillow-rs/src/image/pixel_access.rs: 1 references
- pillow-rs/src/image/projection.rs: 9 references
- pillow-rs/src/image/projection/getprojection_cmyk_tests.rs: 2 references
- pillow-rs/src/image/projection/getprojection_rgb_pipeline_tests.rs: 1 references
- pillow-rs/src/image/projection/putpixel_pipeline.rs: 4 references
- pillow-rs/src/image/statistics/colors.rs: 4 references
- pillow-rs/src/image_utils.rs: 4 references
- pillow-rs/src/lab.rs: 3 references
- pillow-rs/src/ops/analysis.rs: 4 references
- pillow-rs/src/ops/analysis/bbox.rs: 4 references
- pillow-rs/src/ops/analysis/histogram.rs: 6 references
- pillow-rs/src/ops/convert/binary.rs: 2 references
- pillow-rs/src/ops/convert/dispatch.rs: 5 references
- pillow-rs/src/ops/convert/luma16.rs: 3 references
- pillow-rs/src/ops/convert/nonstandard.rs: 10 references
- pillow-rs/src/ops/convert/operation.rs: 1 references
- pillow-rs/src/ops/convert/palette.rs: 4 references
- pillow-rs/src/ops/convert/support.rs: 3 references
- pillow-rs/src/ops/convert/tests.rs: 4 references
- pillow-rs/src/ops/enhance.rs: 6 references
- pillow-rs/src/ops/param_filters.rs: 2 references
- pillow-rs/src/ops/paste/operations.rs: 1 references
- pillow-rs/src/ops/pil_resize/alpha.rs: 8 references
- pillow-rs/src/ops/pil_resize/boxed.rs: 12 references
- pillow-rs/src/ops/pil_resize/boxed/boxed_f_resize_eight_tap_tests.rs: 1 references
- pillow-rs/src/ops/pil_resize/boxed/boxed_f_resize_fma_tests.rs: 2 references
- pillow-rs/src/ops/pil_resize/float.rs: 18 references
- pillow-rs/src/ops/pil_resize/integer_modes.rs: 4 references
- pillow-rs/src/ops/pil_resize/integer_modes/cmyk.rs: 1 references
- pillow-rs/src/ops/pil_resize/integer_modes_tests.rs: 11 references
- pillow-rs/src/ops/pil_resize/luma16.rs: 3 references
- pillow-rs/src/ops/pil_resize/resize.rs: 34 references
- pillow-rs/src/ops/quantize/operations.rs: 4 references
- pillow-rs/src/raster/dynamic.rs: 107 references
- pillow-rs/src/raster/dynamic/transform.rs: 140 references

### Effects-family source progress: incomplete and unvalidated

- Added the `effects` family with separate `EffectSpread` and `EffectNoise` contracts, exact-mode preparation, prepared-operation registry entries, and Default implementations under all 25 supported mode folders. `EffectSpread` preserves each mode's native typed samples and exact output variant. `EffectNoise` reads dimensions from the exact typed input and declares its fixed `ImageLuma8` output.
- Every internal high-depth route is marked `NO_PILLOW_SUPPORT`. `EffectSpread` also explicitly reports its native-preserving extension as core-only for `La`, `LAB`, the four `I;16` identities, and the five internal high-depth identities; the old path converted or did not define those routes. `EffectNoise` accepts any exact mode as a native dimension carrier and returns L; only the L dimension-carrier route corresponds to Pillow's size-based entry point, so the other 24 mode contracts are marked `NO_PILLOW_SUPPORT` and core-only.
- `EffectSpreadDistance` and `EffectNoiseSigma` are typed input contracts. Prepared mode strategies receive the injected `ExecutionContext`. The archived byte-vector Spread path is now under Default and Sse2/Neon/Wasm SIMD128 strategies for its 14 legacy byte layouts; other modes use native typed Default strategies. Noise's vector arithmetic is declared for every exact mode and returns the fixed L output. Both use request guards to select the specialization or the same mode's Default path.
- Moved the process-global Park–Miller sequence and shared spread/noise control-buffer generation to `compute/helpers/effects`, which is used by CPU and GPU. GPU auxiliary-image generation and identity-spread stream advancement now use that shared helper. `affine_nearest_fixed`, also used by CPU transform and GPU planning, moved to `compute/helpers/geometry` rather than remaining hidden behind the old effects facade.
- Reusable vector data planes live in `effects/helpers/{spread_vector,noise_vector}.rs`; each declared mode/architecture entry points to the shared kernel without converting or retagging its input. Native-preserving Spread routes remain explicitly `NO_PILLOW_SUPPORT` for `La`, `LAB`, the four `I;16` identities, and the five internal high-depth identities.
- The registry entries point to the prepared pipeline callbacks, but the maintained CPU pool/executor and remaining eager/compatibility routes are not yet wired to them. The former EffectNoise and EffectSpread SIMD registrations are removed from the active registry. TODO in `pool_cpu/ops/effects/ops.rs`: compare their archived admission and architecture behavior with the CPU mode strategies, then route any remaining eager/compatibility callers through the injected CPU context. Do not treat this family or CPU backend as complete.
- No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage command has run.

### Rotate-family source progress: incomplete and unvalidated

- The new `rotate` owner declares all 25 exact mode strategies and returns the exact corresponding `DynamicImage` identity. Its entry point owns request facts and operation support; each mode's `ops.rs` declares its support and binds a Default implementation. The five internal native high-depth strategies use typed samples and carry nearby `NO_PILLOW_SUPPORT` comments.
- Pillow-defined `1`/`P` retain forced nearest sampling, `LA`/`RGBA` retain straight-alpha premultiplication around filtered sampling, and `La`/`RGBa`/`PA`/`RGBX`/CMYK retain raw stored-channel semantics. I/F filtered Default paths and architecture-selected bilinear paths treat their four-byte carriers as typed scalar samples; nearest paths preserve the raw carrier bytes. I;16 variants retain their endian-specific native storage, and I;16 nearest sampling keeps its archived floating-point Geometry.c coordinate mapping.
- The serial byte nearest kernel restores the three-byte contiguous source-span copy and RGBA four-pixel unroll from the archived CPU implementation. Exact-angle and unexpanded rectangular right-angle routing follows the archived behavior. I;16 filtering retains the archived low-byte filter arithmetic without casting the raster mode to L.
- All byte-backed exact modes now declare AVX2, SSE2, NEON, and Wasm SIMD128 nearest strategies in their exact mode architecture folders, including `LA`, `La`, `PA`, `RGBA`, `RGBa`, `RGBX`, `CMYK`, `YCbCr`, `LAB`, `HSV`, and the raw-byte `I`/`F` carriers. Their request guards select these kernels only for finite, non-identity, non-discrete nearest transforms with exact native bytes; all other requests select the same-mode Default strategy. `One` and `P` retain their forced-nearest behavior, and each result keeps its exact mode identity. The vector kernel gathers native samples into 16-byte blocks and retains scalar tails; three-byte modes also retain contiguous source-run copies. The SSE2 build declaration is disabled when AVX2 is a compile-time target feature, so an SSE2 context cannot reach code built for a stronger ISA.
- The same mode/architecture owners declare the archived eight-lane bilinear kernel for eligible byte modes. Its interpolation and geometry helpers stay in Rotate helpers; `LA` and `RGBA` select the alpha-aware path, while premultiplied/indexed/padded/color-space modes preserve their stored channel bytes. `I`/`F` use their separate typed bilinear vector kernel and retain raw-byte nearest sampling.
- `Image::rotate` and its typed-input wrapper enqueue `PipelineOp::Rotate`; the registry maps that operation to this family's `prepare_pipeline`, and the serial CPU batch executor calls it through `prepare_cpu` with the routed `ExecutionContext`. The public route is therefore connected to mode/architecture preparation; per-mode comparison with every direct, compatibility, and archived behavior remains part of source review. No separate maintained `op_rotate` implementation is active.
- Existing Rotate regression tests are not the migration's verification plan and have not been run. Do not add unit tests for this refactor; the planned verification is parity across each applicable operation × exact Mode × Architecture combination, including vector tails, Default fallbacks, and rejected requests. This family remains source-only and unvalidated; no build, formatter, checkstyle, lint, test, parity, benchmark, or coverage command has run.

### Registry request-support source progress: incomplete and unvalidated

- Split semantic `InputContract` assessment into `registry/support/request/contracts.rs` and materialized `StrategyGuard` checks into `registry/support/request/guards.rs`. The parent module now delegates to these focused helpers and stays below the 1,000-line file limit. Error classes, messages, unresolved-input results, and exact byte-layout conditions were carried over during source review.
- Split the oversized materialized guard implementation into `guards/core.rs` and `guards/mutation.rs`, leaving `guards.rs` as a small export module. The 39 guard declarations retain their existing names and are all re-exported through the path consumed by `StrategyGuard::assess`; each maintained file is below 1,000 lines. This is source-only and uncompiled.
- The support source review is not complete for the repository as a whole; remaining functions and files still need the planned size review after the implementation migration. No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage command has run.

### Put-family source progress: incomplete and unvalidated

- PutAlpha and PutAlphaData now have native typed paths for `I;16*`, internal 16-bit luma/RGB/RGBA, and internal 32-bit float RGB/RGBA. These paths preserve source color samples and emit the declared LA16, RGBA16, or RGBA32F output; they do not route through an 8-bit RGBA carrier. The u8 alpha sample is expanded to the full u16 range or normalized to f32 at the destination sample type.
- The `I` and `F` PutAlpha modes remain explicitly rejected with their existing Pillow error messages. The new I;16 alpha outputs are core-only extensions and carry nearby `NO_PILLOW_SUPPORT` comments.
- PutAlphaData still converts accepted mask modes to L where the operation contract requires luma mask semantics. Admission now rejects mismatched image and mask dimensions before the pixel loop, preserving the existing `images do not match` error instead of silently truncating zipped rows.
- This is a source-only slice. PutData and ExtractBand conversion audits, all callers, and the complete family review remain pending. No build, formatting, checkstyle, lint, test, parity, benchmark, or coverage command has run; do not treat the family or CPU backend as complete.

### Filter-family migration status: in progress and unvalidated

- The maintained `pool_cpu/ops/filter` family has an operation entry point and per-mode support and execution modules. Filter3x3/5x5, byte blur, and byte rank now dispatch through exact-mode Default leaves; blur/rank helpers receive native bytes and dimensions from those leaves and do not inspect `DynamicImage` or `Mode`. The shared `filter/ops.rs` entry point handles admission and exact mode selection only.
- Native I32 3x3/5x5 and rank implementations, plus F32 rank, have been moved into their typed mode owners. The retained I32 5x5 vector kernel is now under `filter/i32/avx2`; its support declaration binds `Filter5x5` to AVX2+FMA build and host capability selection, with Default fallback.
- The retained native LA 5x5 binomial and single-outlier vector kernels are now under `filter/luma_a8/helpers`, outside the NEON folder because their packed `wide` implementation is shared by vector architectures. The LA mode strategy checks those exact requests before its generic native-byte vector loop; the dedicated NEON orchestration remains in `filter/luma_a8/neon`, and unmatched kernels use the same mode's Default strategy.
- The retained LA 3x3 binomial NEON kernel is now under `filter/luma_a8/neon`; its architecture strategy requires the exact request, native two-byte layout, and at least one full vector block, with same-mode Default fallback.
- The retained I32 3x3 four-lane convolution is now under `filter/i32/avx2`, next to I32 5x5. Its AVX2/FMA strategy requires finite coefficients, nonzero finite scale, exact four-byte storage, and an interior vector block; other requests use the same-mode Default path.
- The archived native L 5x5 binomial and uniform integer vector kernels are now under `filter/luma8/helpers`, outside an architecture folder because their `wide` implementation is shared by vector architectures. The L mode strategy invokes them only for those exact requests and the exact one-byte L variant; unmatched kernels continue through the generic vector loop, and Default selection keeps the serial implementation.
- The two Filter unit-test source references that called `pool_simd::ops::native_filter` now point to the mode-owned CPU architecture kernels. No tests or other validation have run.
- `Image::validate_filter` uses the Pillow-owned `Mode` for its mode checks. UnsharpMask now prepares its exact-mode blend and nested GaussianBlur from one injected CPU `ExecutionContext`, executes the composition through the eager CPU strategy receipt, and returns the same mode-bearing raster. Its operation contract exposes the exact radius, percent, and threshold facts. Filter3x3, Filter5x5, GaussianBlur, BoxBlur, and rank operations now expose their typed request parameters alongside their mode support. The RGB 150% portable-wide kernel is under `filter/rgb8/helpers/unsharp_blend.rs`; accepted byte modes use the shared same-mode byte blend as Default. The previously unreachable malformed-storage byte fallback was removed after exact mode admission made the native layout authoritative. Source-only, unverified.
- `registry/registration/filter.rs` now points at the prepared CPU family owner. Direct/eager callers still need context-aware wiring; preserve the separate GPU callbacks while doing so.
- The exact 3x3 binomial request recognizer is shared from `compute/helpers/filters` by CPU Filter3x3, its LA NEON strategy guard, and GPU admission, so backend support checks no longer reach into the CPU operation tree for this common request fact.
- All 13 admitted byte modes execute Filter3x3 through exact-mode Default leaves; P remains explicitly rejected, while I32 keeps its native scalar implementation and F32 remains rejected. Their shared raw-byte helper receives the declared channel count and never infers source mode or output identity. Each leaf reconstructs its own exact DynamicImage variant. The archived generic native-byte vector kernel is now a shared family helper selected by each exact byte mode's declared architecture strategies (SSE2, AVX, AVX/FMA, AVX2, AVX2/FMA, NEON, and Wasm SIMD128) when those builds are available. It preserves the RGB byte-stream kernel, grouped LA/RGBA/CMYK lanes, and the bounded small-uniform control path; LA retains its exact binomial NEON and non-NEON vector fast paths.
- The archived native-byte Filter5x5 vector loop is also shared across all 13 admitted byte modes under their exact mode-owned architecture declarations. The typed vector leaves retain a bounded uniform identity proof and native copy, RGB's large-uniform proof scan and BLUR-ring vector kernel, L's portable wide uniform/binomial kernels, and LA's packed-vector single-outlier/binomial kernels with its dedicated NEON strategy. Non-identity uniform inputs continue through the selected vector strategy; RGBA's exact uniform-kernel fast path remains in its Default leaf. `NO_PILLOW_SUPPORT` remains attached to native extensions; P remains rejected, and I32 retains its separate typed AVX2/FMA route. This source-only migration has not been compiled or verified.
- Filter scope now distinguishes Pillow-defined routes from native extensions: Pillow's Kernel contract is L/RGB, BoxBlur and GaussianBlur use Pillow's supported uint8 mode set (L, LA, La, RGB, RGBA, RGBa, RGBX, CMYK), and rank filters use scalar modes (1, L, I, F). Other admitted strategies use `ModeSupport::cpu_core_only` with nearby `NO_PILLOW_SUPPORT` comments; rejected modes remain rejected. This contract classification is source-only and unverified.
- The archived L radius-one BoxBlur and L/LA/RGB GaussianBlur specializations remain under their exact mode owners. A new `FilterNativeByteBlur` guard exposes the general BoxBlur, BoxBlurXY, and GaussianBlur vector strategy for the exact L/LA/RGB/RGBA/RGBa/RGBX mode declarations that the legacy native-byte adapter admitted. The shared helper preserves the eight-lane fixed-point rolling recurrence, replicated edges, separate horizontal/vertical passes, no-op vector-copy admission, and same-mode Default fallback; it contains no Rayon scheduling. L, LA, and RGB keep their radius-one kernels, while other eligible radii use the shared recurrence. The mode and architecture selection is retained through execution context checks. Source-only and unverified.
- RGBA GaussianBlur retains its radius-seven NEON row kernel and AVX2 horizontal kernel plus prepared AVX2 transpose. The AVX2 declaration keeps its target-feature kernel available to runtime capability selection; the exact RGBA mode owner chooses it only for the guarded radius-seven request. The archived 256×256 large-uniform identity proof and vector prefix test now sit beside the RGBA helpers; neither test has run. The shared fixed-point box-parameter calculation feeds guards and execution. Source-only and unverified.
- The archived L 3x3 order-statistic kernel now lives under `filter/luma8/helpers/rank3x3.rs`; MedianFilter(size=3), RankFilter(size=3, rank=1), and RankFilter(size=3, rank=4) select it only for exact native L bytes and nonempty images. Other odd L rank windows through 15x15 use `filter/luma8/helpers/rank_window.rs`; larger windows retain same-mode Default. Both architecture helpers preserve the bounded small-uniform identity path, and mode orchestration matches the exact L raster once before calling typed helpers. The comparison network, second-smallest selector, general rank selection, replicated-edge gathers, and output identity were reviewed against the archived L SIMD path; this source-only migration is unverified.
- The archived F32 8-lane order-statistic kernel now lives at `filter/f32/helpers/rank_vector.rs`. The F32 MedianFilter, MaxFilter, MinFilter, and RankFilter declarations select it for exact four-byte F storage, nonempty images, odd windows through 9x9, and valid ranks; other windows retain F32 Default. The exact-bit uniform proof is shared by F32 Default and the architecture strategy. The kernel reads and writes native F samples and reconstructs the exact ImageF32 identity. Its window gather, order-statistic sorting, edge replication, uniform identity path, and telemetry were reviewed against the archived SIMD adapter; this source-only migration is unverified.
- Remaining family work: complete per-operation support/input/output contracts and confirm every declared native extension has a nearby `NO_PILLOW_SUPPORT` comment; compare the migrated blur/rank paths and each Filter5x5 Default path against preserved algorithms; audit public Filter callers through the prepared registry route. The archived native-byte MaxFilter/MinFilter and non-L byte MedianFilter/RankFilter kernels now have exact-mode architecture declarations, native-layout/window admission, and same-mode output construction in the maintained Filter family; these source changes remain unverified. UnsharpMask's nested GaussianBlur now uses the same injected context and the mode-aware prepared filter owner. The maintained architecture strategies now include shared byte Filter3x3/Filter5x5 vector kernels, RGB's Filter5x5 BLUR-ring kernel, the packed LA binomial 3x3 vector fast path for non-NEON architectures plus its dedicated NEON strategy, portable wide L uniform/binomial Filter5x5 kernels, packed-vector LA single-outlier/binomial Filter5x5 kernels plus its NEON strategy, I32 Filter3x3/Filter5x5 AVX2+FMA, L rank windows through 15x15, and F32 order statistics. Keep Rayon in the later Parallel CPU owner. All work remains unverified; no build, formatter, checkstyle, lint, test, parity, benchmark, or coverage command has run.
- `compute/execution.rs::execute_prepared_shared_with_palette` now obtains Filter3x3's concrete selection from the CPU registry before attempting I32 Arc sharing. The I32 mode owner uses its selected Default scalar proof or AVX2/FMA packed-byte scan, and the non-identity route executes that same prepared plan. The separate SIMD identity helper and backend-based SIMD predicate are no longer used by maintained execution. This remains source-only and unverified; public Filter caller and compatibility-route review is still pending.

### Core image mode API progress: incomplete and unvalidated

- Materialized pixel access, scalar storage validation, statistics, bbox, and histogram dispatch now consume the exact Mode derived from DynamicImage. Rust putpixel mode APIs take Mode; Python putpixel and raw-data wrappers use mode_identity and enum matches. Scalar color-count helpers also receive Mode instead of I/F spellings. Pillow strings remain at public metadata and error boundaries.
- The same source-only pass converted transparency/band reporting, palette checks, analysis masks, rotate filter decisions, paste mask decisions, and ImageOps fill resolution to exact Mode matches. Color names still cross the existing color parser boundary with the canonical Pillow spelling. These changes preserve each existing mode case, including legacy scalar-fill behavior.
- Encoder rejection checks, quantize source/palette checks, PILfont glyph-mode validation, unpacked byte export, and XBM bitmap validation now also use exact Mode. Explicit raw encoder mode arguments remain Pillow strings because they are codec/packer inputs.
- Encoded header ImageMode metadata now translates directly to Pillow-owned raster Mode in the image adapter; it no longer round-trips through a Pillow spelling just to recover the enum.
- Convert now takes typed source and destination identities in core, and enhancement, paste source preparation, and module Composite no longer recover a destination mode string before conversion. Python and JavaScript parse valid external strings before delegation. Raw-byte codec overrides remain string inputs at their explicit codec boundary.
- This is a targeted source migration, not a claim that every image, codec, or operation boundary is typed. Remaining mode-string routes still need inventory, implementation, and error-order review. No build, formatting, checkstyle, lint, test, parity, benchmark, or coverage command has run.

### Resize mode-boundary source progress: incomplete and unvalidated

- Public resize now resolves the exact `Mode` through `Image::mode_identity()` before selecting nearest behavior for `1`/`P`, the `LA`/`RGBA` reducing-gap exception, and the `F` nearest identity shortcut. This uses a planned pipeline identity when available and materializes only when the graph cannot provide one. The chosen identity is passed into resize policy rather than inferred again.
- `apply_reducing_gap` now receives typed `Mode` and checks the four `I;16*` identities explicitly instead of using a Pillow-string prefix. Thumbnail passes its exact mode to its nearest-filter decision, including PA.
- Prepared Resize/ResizeBoxed now dispatch all 25 mode identities to exact typed owners. Public direct Resize queues typed operations, and nested geometry/Thumbnail callers bind prepared children with the same context. `ops/pil_resize` provides shared low-level helpers; its generic full-operation wrappers are not active prepared CPU callsites. The prepared Default route now performs Pillow's exact boxed identity/integer-crop shortcuts; architecture guards select that route for those cases, while filtered straight `LA`/`RGBA` crops retain alpha resampling. Float32 bounds are narrowed at validation, guard, and coefficient boundaries; unchanged-axis pass requirements are shared with the native typed-float strategy. Source review confirms the native I;16 owner returns correctly shaped typed buffers across empty dimensions; the public parity matrix must confirm these behaviors. Validation/error ordering, legacy-wrapper reachability/cleanup, and full source review remain pending. Source-only; no build, formatter, checkstyle, lint, unit test, parity, benchmark, or coverage command has run.
- The archived Luma byte convolution now lives in `resize/luma8/helpers/vector.rs`, with exact SSE2, NEON, and Wasm SIMD128 entry points under `resize/luma8/<arch>/resize_impl.rs`. Its checked coefficient guard selects these strategies only for nonempty, unboxed, filtered Luma8 work with native storage and i32-safe taps; the same two-pass fixed-point rounding and scalar tails are retained. Pad's full-width window can execute the selected Luma8 architecture strategy directly. The archived ordinary native-byte vector convolution, its bounded i32 fast paths, and coefficient lane plan now live in `resize/helpers/native_byte_vector/`; eleven exact byte-mode owners provide SSE2, NEON, and Wasm SIMD128 filtered entry points, with channel and alpha policy selected as compile-time mode parameters. Straight LA/RGBA preserve their premultiply and unpremultiply path; premultiplied alpha, PA, and raw color-space samples remain native. Their declarations use the same checked nonempty, unboxed, i32-safe coefficient guard and retain the same-mode Default fallback. Boxed byte convolution and nearest-vector paths now have exact byte mode/architecture owners with float32 bounds and coefficient admission. Exact integer crops and identity boxes use the Default shortcut; filtered straight-alpha crops continue through their selected resampler. One and P remain nearest-only vector strategies. The migrated code contains no Rayon scheduling. Boxed validation/error-order review and generic-wrapper reachability cleanup remain TODOs in `resize/ops.rs`. This is source-only and unverified.
