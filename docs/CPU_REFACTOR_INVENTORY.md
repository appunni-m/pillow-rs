# CPU refactor source and evidence inventory

Status: baseline inventory captured; the 98 CPU and 182 SIMD files are preserved below pool_cpu/legacy; maintained implementation and source review remain in progress. The separate SIMD executor and adapter registry were removed from the active module graph on 2026-10-31; `Backend::Simd` is now a CPU-plus-Architecture compatibility request. RGB8 Wasm SIMD Convert strategies are restored from archived portable-vector routes, Default RGB→LA/I;16 staging preserves the archived L scratch allocation, and reviewed Convert empty routes retain their archived receipts. The RGBA/RGBa-to-LA vector kernel now lives in a shared native four-byte helper used by both exact mode strategies. The RGBX-to-La/RGBa routes and their inventory were committed in `baf192167`; `0b34e9a9b` exposes same-mode Convert identity in all 25 source contracts, including palette-preserving P/PA and CoreOnly internal high-depth identities. `e75e87f43` routes named ImageOps/Paste color resolution through exact `Mode` and marks internal high-depth color results `NO_PILLOW_SUPPORT`. `7790ed967` classifies reviewed mode-preserving pipeline operations, records RemapPalette and Color3DLut output identities from typed operation contracts, and routes Tiny-RGB eager ExtractBand through prepared CPU architecture execution for CPU Default and compatibility SIMD. `054a98326` replaces clone-only same-carrier PutData prefix writes with the native writer in 16 mode owners, removes copy-only carrier conversion from four PutAlpha and 13 PutAlphaData Default paths, and makes callback-preserving PutData writes validate and mutate the exact materialized DynamicImage byte carrier. `aa6de79ae` carries exact `raster::Mode` through PutData value-kind selection, normalization, immediate-write width selection, and the deferred `PipelineOp::PutData` payload. `3f110aa47` adds callback-driven PutData's exact-mode support contract and cached CPU Default context; this worktree now routes synchronous PutData and PutPixel selections through image-pinned/active CPU Architecture policy, with native Default fallback where no specialization exists. `65fb1c21c` adds mode-owned synchronous PutPixelValueAt for numeric I/F and I;16. `e060b60c1` adds the CPU Default strategy for Pillow's raw-byte I;16 PutData path. `751fe8249` routes allocated GetData band reads through the caller-owned output path. Commit `3254bd6ad` moves GetData byte, band, I;16 scalar, and I/F scalar algorithms into exact-mode Default owners for all 25 identities. Commit `572b7fd6c` carries exact `raster::Mode` through PutAlpha and PutAlphaData descriptors, while GPU packing preserves the existing shader layout codes. Commit 11665ed82 passes typed Mode through prepared execution and array import. The lazy-mode-cache consolidation is source-only and unverified; cross-mode P/PA terminal palette metadata remains incomplete. The active implementation remains incomplete; see the latest check evidence below.
Baseline commit: 3f9dfd4036caa061eabcf2255c9f34a745df9c7e
Working checkout: `/Users/lazytrot/work/pillow-rs`, branch `develop`. Static
lazy output modes now come from operation support contracts without
wrapper cache overrides; mode metadata remains source-reviewed and
runtime-unvalidated.
Latest CPU-only compiler check (2026-11-03, source at commits `a1ed8e869` and
`b536c0e8a`): no E-coded type/name errors were emitted, but `cargo check`
failed on denied lints (120 `dead_code`, 335 `unused_imports`, 75 `deprecated`,
18 `unsafe_code`, 3 `unused_macro_rules`, 2 `unused_variables`, and 1
`unreachable_patterns`; 40 additional warnings). The compiler reached lint
evaluation, but this is not a successful build. No formatter, separate lint
target, tests, parity, benchmark, or coverage ran for these commits.
Commit `9af54f318` subsequently removed the audited orphaned helpers and
per-mode wrappers listed in the plan; that source-only cleanup has not been
compiled. Commit `970bd036d` gates Composite architecture modules with the
same target conditions as strategy dispatch. A static source scan found other
architecture module declarations that still need family-by-family target
reachability review; preserve them until their support declarations and
dispatch conditions are checked. The cross-family review remains TODO in
`pool_cpu/ops/mod.rs`.
Commit `8bad284fe` retains the exact `One` or `ImageLuma8` identity of a
PutAlphaData mask through CPU preparation and updates the corresponding GPU
auxiliary admission and native byte-mask routes. It is source-reviewed but
runtime-unvalidated. Commit `de3f7c903` corrects PutAlpha's declared mode and
removes redundant alpha-mask copies for exact One/L/P carriers. The commits
include the exact-mode PutData callback writer and its
sequence-scoped CPU Default context, mode-owned numeric I/F and I;16
PutPixelValueAt strategies, four exact-mode Default owners for Pillow's
row-shaped raw-byte I;16 PutData path, and explicit GetData/GetDataBand support
contracts with backend selection, and the committed GetData algorithms under
all 25 exact-mode Default owners. GPU maps exact payload modes to established
shader byte-layout codes; PutAlpha and PutAlphaData now carry exact `Mode`
fields. The GetData source review and all other current CPU source
remain uncompiled and unvalidated. No build, format, lint, test, parity,
benchmark, or coverage validation has run.
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

- Pillow 12.2 `ImageOps._border` source review confirms that a tuple of length
  two duplicates horizontal and vertical margins, length four unpacks directly,
  and other tuple lengths reach uninitialized local variables. Non-tuples are
  repeated as-is; `expand` performs Python size addition before `_color`, then
  palette lookup/allocation, `Image.new` validation, and Paste. The current
  Python size arithmetic and fill/palette resolution before size validation.
  The facade now loads the source after two-coordinate box arithmetic and
  before native coordinate parsing, matching `Image.paste` while retaining a
  lazy Expand result. Remaining source work covers callback-driven wrapper
  replacement and eager allocation failure timing; dirty/malformed palette
  behavior remains parity work. Sources: [Pillow 12.2 `ImageOps.py`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L30-L38),
  [expand](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L446-L473),
  and [paste](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L1692-L1773).
  Source-only; no validation ran.
- Pillow's encode adapter now preserves codec transfer identities for `1`, `I`,
  and `F`, and explicitly serializes native u16 carriers as the pinned codec's
  little-endian transfer bytes for I;16 and internal 16-bit modes. The adapter
  still has a mode-specific encode TODO for PA's palette indices and
  per-pixel alpha, premultiplied alpha, RGBX, YCbCr, LAB, and HSV. The generic
  LA transfer cannot preserve PA identity or palette metadata. These
  codec-owned cases are tracked in `image-slash-star#9h5w`; append additional
  mode/format gaps to that Kata task as the adapter audit discovers them, and
  keep Pillow changes at the codec boundary. The dependency remains pinned by
  `pillow-rs/Cargo.toml`; no files
  in that repository were changed. Source-only and unvalidated (2026-11-03).
- Mode-boundary review confirms `Mode::ALL` and the `DynamicImage::mode()`
  variant match cover the same flat 25 identities; `ColorMode` is an alias of
  `Mode`, and codec `ImageMode`/`ColorType` translation stays in
  `image/codec_adapter.rs`. Python and JavaScript binding entry points parse
  Pillow strings before typed Rust construction and operation calls. A
  production-source scan found no mode strings in maintained `pool_cpu`; its
  remaining `ColorType` reads are storage channel/byte counts in Histogram and
  Transpose, not mode selection. The indexed-filter audit found the PA
  BoxBlur/BoxBlurXY and rank-filter declarations had native two-channel
  strategies, but materialization expanded them to RGBA before dispatch. The
  source-aware planner now keeps those exact PA operations in native PA; the
  PA GaussianBlur rejection is unchanged. The P Equalize strategy also had a
  fixed RGB output contract and palette input, but generic materialization
  expanded P before it could select that strategy. CPU/CPU-architecture
  materialization now dispatches the original P raster and palette through the
  declared strategy, whose RGB conversion drops palette alpha; PA Equalize
  remains rejected and is no longer planned as RGB. GPU Equalize retains its
  existing generic route for the later backend review. Remaining
  color-dependent indexed transitions still defer to materialization pending
  operation-specific output contracts. GPU pending/result metadata also
  retains a string route for the later backend stage. These boundaries remain
  source-only and unvalidated.
- Core raw-byte export and converted compatibility metadata now accept typed
  `Mode`; Python and JavaScript parse Pillow's external mode strings at their
  binding boundary. Histogram and Entropy masks are admitted only as exact
  `One` or `ImageLuma8` identities and their byte planes are borrowed directly
  during scans. ImageOps Autocontrast likewise reads those accepted native
  mask bytes without the previous `to_luma8` clone, while retaining luma
  normalization for any internal caller that supplies another mode. I;16
  GetPixel now reads the native `u16` carrier instead of allocating a
  converted Luma16 image. These source changes remain uncompiled and
  unvalidated.
- Empty-image source review found the archived SIMD Chops Invert and
  RemapPalette routes accept zero-byte rasters and retain their native-copy or
  scalar-control receipts. Their maintained kernels also handle exact empty
  storage, but the shared `NativePointwiseBytes` guard rejected every zero
  dimension before those strategies could run. Added the distinct
  `NativePointwiseBytesIncludingEmpty` guard for ChopsInvert and RemapPalette;
  the original nonempty guard remains in use for Eval and the shared ImageOps
  point-strategy callers; their empty-image routes were not changed in this
  source review. ChopsInvert remains an exact-mode architecture route with
  same-mode Default fallback. This source correction is uncompiled and
  unvalidated.
- Chops caller tracing now covers the public typed mode-group validation,
  lazy `PipelineOp` construction, `prepare_plan` secondary materialization and
  support assessment, and exact-Mode prepared dispatch. Binary shape mismatch
  stays on the serial Default strategy, which clips to the overlap; the
  architecture guard admits owned Add/Sub execution only for equal native
  layouts, and the in-place path is restricted to selected vector Add/Sub.
  Multiply→Screen preparation remains limited to L/LA/RGB/RGBA, a shared
  execution source, matching mode and dimensions, and matching selected
  architectures. The `Backend::Simd` compatibility request is also source-
  traced: the backend registry has `CpuPool` (and optional `GpuPool`), not a
  SIMD pool; routing maps the alias to a CPU context with the detected
  architecture, and the Python ImageChops bindings call the typed Rust
  operations. For Pillow-defined modes, binary wrapper validation still
  resolves both input identities before reporting source/secondary
  incompatibility, matching the baseline; LAB remains explicitly CoreOnly.
  Logical validation rejects the source mode before reading the secondary.
  Replaced the custom wrapper mode lookup, which could materialize an
  inferable pipeline, with `Image::mode_identity()` so cached and planned
  typed modes remain lazy. Owned Add/Sub and Multiply→Screen batch fallback
  and receipt paths, public direct callers, lazy output-mode planning, and
  validation/error order are now source-reviewed against the baseline. No
  validation has run.
- Per-operation telemetry now includes stable typed strategy fallback names,
  separate from pipeline backend fallback. Prepared/eager, reviewed fused, and
  vector PutPixel batch receipts record the actual `StrategySelection`. Host
  terminal receipts still record only Architecture; a source TODO tracks
  selection plumbing. The optional fallback list is absent when no selection
  was published and empty when a recorded selection had no fallback. This
  telemetry edit is source-only and unvalidated.
- `ModeSupport::assess` and `select` now examine permitted lower architectures
  even if the requested architecture is unavailable on the captured host, and
  reject any candidate whose own architecture or extra instruction
  requirements are unavailable. A selected lower route retains the requested
  strategy's hardware fallback reason. This dispatch correction is source-only
  and unvalidated; exact/lower/hardware cases remain in the CPU parity matrix.
- Direct `GetData`, allocated `GetDataBand`, and caller-owned `GetDataBandInto`
  have independent exhaustive support descriptors under `analysis`; band-count
  facts and the existing scalar-mode caller-buffer rejection come from those
  declarations. Exact-mode Default owners now cover all 25 identities under
  `analysis/<mode>/default/get_data_impl.rs`. `image/data_access/get.rs` retains
  validation, materialization, backend routing, and result shaping; it no longer
  owns byte narrowing, LAB mapping, interleaved band extraction, or I/F scalar
  decoding. Shared native-sample helpers preserve the existing clipping,
  wrapping, byte order, and channel-stride behavior without retagging or
  converting modes. Zero-copy One/L/P views also use their mode owners.
  Allocated non-scalar band reads preserve eager materialization and band
  validation order. SIMD's direct sink path remains in use for the nine
  eligible layouts. GPU terminal eligibility remains a TODO in
  `analysis/ops.rs` pending a shared GPU support contract. This source is
  uncompiled and unvalidated.
- `PutAlpha` and `PutAlphaData` now carry exact source `Mode` values in
  `PipelineOp`, and the duplicate `PixelMode` enum is removed. The image wrapper
  retains its existing accepted/rejected modes; CPU preparation checks that the
  captured source mode equals the materialized `DynamicImage` identity. GPU
  admission, output handling, native routes, and shader packing use exact
  identities while retaining existing numeric layout codes. This source-only
  change remains uncompiled and unvalidated.
- Pipeline output metadata uses the typed mode planner instead of retaining a
  non-paletted source identity by default. Source tracing found Colorize nodes
  cached L despite returning RGB, and EffectNoise nodes could retain an
  arbitrary input mode despite returning L; either stale declaration could
  force materialization to retag the result incorrectly. For indexed inputs,
  palette-safe P/PA handling remains explicit. Known mode-changing routes use
  their declared transition; mode-preserving routes that are not palette-safe
  defer identity to materialization rather than claiming the source mode. A
  source TODO tracks replacing those indexed fallbacks with exact transitions
  or existing rejections. Merge and ExtractBand now seed the output cache from
  their typed operation contracts, and Image::merge no longer reapplies a
  partial target-mode list after queuing. This source-only correction is
  unvalidated.
- Prepared execution, its shared-result variants, and `BackendImpl` now carry
  `Option<Mode>` instead of a Pillow spelling. Materialization resolves the
  logical source identity with `Image::mode_identity()` and keeps palette mode
  choices typed; CPU derives implementation selection from the exact
  `DynamicImage` variant and ignores the separate logical compatibility value.
  GPU admission, planning, and upload still use `Option<&str>` logical-mode
  inputs. Replace those with exact `Mode` matches and remove the pool adapter;
  this remaining GPU route is covered by a TODO. The CPU/backend interface
  migration is source-only, uncompiled, and unvalidated.
- `from_resolved_array_interface` now uses the already-resolved
  `ArrayLayout.mode` as its output identity. `ArrayLayout.raw_mode` continues to
  describe the source byte representation; core no longer parses that spelling
  back into `Mode`. This source change is uncompiled and unvalidated.
- `Image::Pipeline` now has one `PipelineModeCache` instead of separate
  `explicit_mode` and mode-cache fields. The cache distinguishes an
  graph-declared output identity from a mode inferred by metadata planning;
  materialization, palette loading, pipeline admission, and GPU graph metadata
  consume the appropriate state. This keeps `Image::mode_identity()` exact
  while preserving the old declared-mode behavior for execution. Source-only,
  uncompiled, and unvalidated.
- Callback-preserving `Image::putdata_value_at_with_context` selects the declared `PutDataValueAt` support contract for the exact mode and writes through the native byte or I;16 carrier. The Pillow and JavaScript sequence adapters retain one routed CPU Architecture context for their callback-driven writes; the current contract has no non-Default specialization, so SIMD requests resolve to the native Default owner and report that fallback. The context caches the selected mode strategy and refreshes it if a re-entrant callback changes the image mode. The synchronous mutation remains visible before the next value is coerced, and telemetry records the selected implementation alongside any requested-backend handoff. The direct `Image::putdata_value_at` API creates the same context for one write, while callback-free I;16 value loops reuse one context. Source review against the prior implementation confirms typed normalization order, synchronous mutation, exact mode identity, `LoadedData` metadata retention, palette retention, and existing storage/offset errors on valid rasters. This immediate route has no cross-mode conversion; broader wrapper review remains open, and source is unvalidated.
- Numeric I/F `Image::putpixel_mode_scalar` now selects the distinct `PutPixelValueAt` contract from the exact raster mode instead of reporting itself as PutData. It retains the existing numeric-to-four-byte normalization and in-place write through the mode-owned I/F Default implementations, and records the selected implementation from the routed CPU Architecture context, with native Default fallback until specialized owners exist.
- Direct I;16 PutPixel now uses that same contract with four exact mode classifications and separate `i16`, `i16l`, `i16b`, and `i16n` Default owners. Each owner writes the typed u16 sample in place, preserving modulo-65536 normalization, byte-order identity, and the existing offset error. Both routes use the shared routed context. The broader wrapper/source review and all validation remain pending. These PutPixel updates are unverified.
- Raw byte export now accepts the exact `Mode` in Rust. Python and JavaScript bindings obtain/pass that typed identity before calling the core; raw encoder names such as `BGR` and `BGRA` remain raw-layout strings at the host-facing boundary. Core conversion-metadata queries likewise accept `Mode`, with Python/JavaScript target strings translated in their bindings. The byte-export I;16 endian guards now match typed identities instead of parsing Pillow strings. Formatted I;16 pixel reads access the exact u16 carrier directly and avoid the full-buffer clone from `to_luma16()`. Existing call sites were updated without adding tests or running verification; source-only and unvalidated.
- PutAlpha keeps One/P source luma borrowed until the required two-channel output is made, updates PA indices and alpha directly in its native two-channel buffer, and replaces La alpha in its native two-channel buffer. PutAlphaData reads One/P/PA/La directly, uses native three-channel interleaving for RGB-family inputs, and passes existing four-channel source buffers to the alpha-mask writer without cloning. The archived cross-mode audit confirms that CMYK-to-RGB promotion and mask luma semantics are preserved; exact-mode source changes preserve the legacy output carriers and arithmetic. Accepted One/L mask buffers now retain their identity and lend luma bytes to Default alpha paths without allocating a duplicate L frame. Required destination copies remain. This source review has not been validated.
- PutData's 16 clone-only same-carrier routes now use `put/helpers/put_data.rs::native`. For complete payloads, the helper constructs the output directly from the supplied bytes; partial and empty payloads retain the source buffer and apply the same byte prefix. This is equivalent to the prior `image.clone()` plus `Prefix::apply` path because those closures performed no sample or mode conversion. The remaining `ConvertBuffer` branches were reviewed against the archived cross-layout behavior and are retained because payload width changes the output carrier before prefix replacement; no conversion remains only for kernel reuse. This source review has not been validated.
- PutData input-shape selection, byte normalization, and callback-path storage width match exact `raster::Mode` identities. The byte layouts remain mode-specific: One/L/P are one byte, LA/La/PA two, RGB/YCbCr/LAB/HSV three, and RGBA/RGBa/RGBX/CMYK/I/F use their existing four-byte carriers. The deferred `PipelineOp::PutData` carries that exact identity as `payload_mode`; CPU contracts and GPU admission, packing, and shader-code selection consume it directly. Internal high-depth payload modes retain their explicit rejection; Pillow La and LAB use native two- and three-byte carrier strategies. This source review has not been validated.
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
  tag on its index raster. The lazy P result retains its declared output mode because it must
  restore the palette wrapper when loaded.
- Binary conversion and font-mask construction now create the exact `One`
  variant directly. Quantize's lazy P source likewise uses `ImagePalette8`;
  the pipeline retains its declared P output identity for palette restoration.
- Native two-channel readers used by GetData, GetColors, GetExtrema, Entropy,
  Histogram, GetBBox, and GetProjection now match palette-alpha and
  premultiplied-luma-alpha variants where they share the same raw channel
  semantics. These are focused carrier-preservation edits; they do not finish
  those operations' exhaustive mode contracts.
- `Image::getchannel`, `Image::putalpha`, and `Image::putdata` now use exact
  `raster::Mode` for mode decisions. The I;16 `putdata` byte order is selected
  from the exact I;16 variant. `putalpha_data` validates its materialized mask
  by its exact variant. `PutAlpha` and `PutAlphaData` now also carry exact
  `Mode` values. GPU shader packing retains only a numeric layout-code mapping
  at the device boundary; it does not define a second mode enum. These
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
  `known_mode_identity()`. A pending declared pipeline mode therefore cannot override
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
  high-depth samples, without widening the whole raster to RGBA. Its byte-mode
  path now selects GetDataBandInto once and carries the image-pinned/active
  backend and CPU Architecture policy through SIMD/GPU terminal selection or
  native CPU fallback; GPU terminal eligibility remains a wrapper TODO. Scalar
  modes retain their direct GetDataBand conversion owner. Typed band formatted
  output and scalar extraction also use `Mode`. Source-only; unverified.
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

### Paste source audit (maintained strategies written; source review in progress, runtime validation pending)

- Public admission lives in `pillow-rs/src/ops/paste/operations.rs`. It reads
  the destination's typed `Mode`, validates placement and source/mask region
  dimensions, derives `mask_alpha` from the exact mask mode, then queues
  `PipelineOp::Paste`; output preserves destination identity and metadata.
  `1`/`L` masks use their luma sample; `LA`/`RGBA`/`RGBa` masks use the alpha
  sample, with `RGBa` retaining its premultiplied PREBLEND rule. Other masks
  return `ValueError("bad transparency mask")`. `resolve_image_source`
  preserves same-mode inputs, preserves `LA`/`RGBA`/`RGBa` carriers for an RGB
  destination, and promotes P indices to opaque PA. Other conversions are
  explicit at this wrapper
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
- I;16 source review (2026-11-01): the maintained Default and SIMD helpers
  match the archived CPU and eight-lane routes for unmasked full-sample copy,
  binary whole-sample copy, and fractional one-byte-per-pixel blending. The
  selected stored byte alternates by offset and follows I;16/I;16L/I;16B/I;16N
  byte order; the other byte is preserved. Fractional L/alpha masks use the
  archived DIV255 or RGBa PREBLEND formula, including PREBLEND at mask zero.
  SIMD groups of eight and padded tails retain the archived vector-block and
  scalar-tail receipts. This is source review only; runtime parity remains
  pending.
- The archived SIMD adapter is `legacy/pool_simd/ops/adapters/paste/`. Its
  native-copy planner covers the 14 byte identities `1`, L, P, LA, PA, RGB,
  HSV, YCbCr, RGBA, CMYK, RGBa, RGBX, I, and F, plus all four I;16 identities.
  (La and LAB have CPU Default behavior but no SIMD Paste plan.) Its planner
  admits binary-mask copy for every listed layout and fractional masks for
  L/LA/PA/RGB/HSV/RGBA/CMYK/I;16. It includes a `Broadcast` source layout for
  the 1x1 masked bitmap source and direct unmasked RGBA-to-RGB and RGB-to-RGBA
  routes. Its byte kernels use 8/16-lane `wide` blocks. Unmasked copies keep
  scalar tails; masked public Paste and Composite routes pass the short-tail
  flag and use padded vector blocks. Explicit AArch64 NEON blocks exist for
  L/LA/RGBA/RGB/CMYK L-mask blends. I;16 uses
  eight u16 lanes and preserves the selected stored byte. Its adapter also
  contains `simd_composite_module`, which reuses these native mask/copy kernels;
  the maintained CompositeModule owner now reuses the matching exact-mode Paste
  architecture strategy through a borrowed native-byte plan.
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
  The exact L-mask NEON subkernel admission is now in seven mode support
  declarations: same-mode native source, nonbinary L mask, no compact
  broadcast, nonempty clipped overlap, and clipped row-width minima of 16
  pixels for L, 8 for LA, 1 for PA, 16 for RGB and HSV, 4 for RGBA, and 16
  for CMYK. The generic architecture vector strategy remains selectable for
  other Paste inputs. The
  archived Rayon clipping case moves to `pool_parallel`. Top-level caller
  wiring is reconciled; per-mode implementation and support review is still
  open, so this is not full family verification. This source-only
  migration slice is unverified; no build,
  formatter, checkstyle, lint, test, parity, benchmark, or coverage command
  has run for Paste.
- Pillow 12.2 Paste source follow-up: the Python wrapper skips conversion
  for `LA`, `RGBA`, or `RGBa` sources pasted into `RGB`; the historical change
  says this avoids unnecessary conversion work. Other differing modes are
  converted first by the base wrapper, but an overridden `convert()` result is
  trusted without checking its returned mode. `ImagingPaste` then checks only
  dimensions and C `pixelsize`: 1 byte for `1`/L/P, 2 bytes for I;16 aliases,
  and 4 bytes for LA/La/PA/I/F/RGB/RGBA/RGBa/RGBX/CMYK/YCbCr/LAB/HSV. An
  unmasked paste copies all physical bytes; masked loops blend all physical
  lanes according to the mask path. Rust's compact LA/La/PA and RGB/YCbCr/LAB/
  HSV carriers omit some lanes, so the Pillow-owned raster now retains those
  lanes and the resolved-source Paste plan admits equal-width byte layouts.
  Normal base-image mode conversion still happens before this raw-layout
  strategy. The 1-byte, 4-byte, and typed I;16 raw paths are source-implemented
  but unvalidated. I;16 source review now covers all four identities,
  byte-order translation, masks, clipping, and cross-endian vector blocks;
  producer-specific lane retention across other mode-changing operations
  remains part of the source audit.
  Sources: [`Image.paste`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L1756-L1764),
  [`Storage.c` pixel layout](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Storage.c#L1848-L1910),
  [`Paste.c` pixel-size check and raw copy](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Paste.c#L270-L304),
  [Pillow 1.0b2 performance change](https://github.com/python-pillow/Pillow/blob/12.2.0/CHANGES.rst#L2845-L2849).
- RGBa-to-RGB Paste source audit (2026-11-03): Pillow does not convert
  `RGBa` to straight `RGB` in this route. The Python wrapper skips conversion,
  and `Paste.c` copies the source's first three physical bytes into the RGB
  destination's visible channels (or blends those bytes for a mask); the
  samples remain premultiplied. The separate `Convert.c::rgba2rgb_` formula
  applies only when `convert("RGB")` is actually called. The RGB source layout
  therefore maps physical lanes directly and does not unpremultiply. Sources:
  [`Image.paste`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L1756-L1764),
  [`Paste.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Paste.c#L301-L329),
  [`Convert.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L2990-L3020).
  Source-only; no build, formatter, checkstyle, lint, test, parity, or benchmark ran.

This inventory records the source and evidence denominator before rebuilding pool_cpu. It is a migration ledger, not a claim that the listed mode literals prove support. Each function's exact mode support, output identity, rejection behavior, shared helpers, and architecture fallback must be confirmed from its implementation before it is copied into the maintained tree. The source path lists are taken from the baseline Git tree, so every archived file can be checked against 3f9dfd403.

## 1. Baseline and preservation boundary

- pool_cpu: 98 tracked Rust files: 2 pool-level files and 96 files under ops.
- pool_simd: 182 tracked Rust files: 4 pool-level files and 178 files under ops.
- PipelineOp: 84 baseline variants assigned to 21 operation families. Exact variant ownership is frozen in Section 5 of IMPLEMENTATION_STRUCTURE.md.
- No pool_parallel existed at baseline.
- At the baseline, raster::DynamicImage has ten storage variants and no exact mode tag. The public Pillow image wrapper carries logical mode outside that enum. The maintained enum expansion is in progress and has not been compiled or validated.
- The baseline raster storage variants are ImageLuma8, ImageLumaA8, ImageRgb8, ImageRgba8, ImageLuma16, ImageLumaA16, ImageRgb16, ImageRgba16, ImageRgb32F, and ImageRgba32F. This layout cannot distinguish multiple Pillow identities sharing a storage buffer.
- Current mode carriers to reconcile: `LoadedData` derives identity only from its exact `DynamicImage` variant. `Image::Bytes` caches a non-optional Pillow-owned `Mode` projected from image-slash-star header metadata; materialized execution takes the `DynamicImage` variant as authority. `Image::Pipeline` has one `PipelineModeCache` with `Declared(Mode)` and `Planned(Option<Mode>)` states; `explicit_mode_identity()` exposes only the declared state, while `known_pipeline_op_mode` populates planned state. Pipeline mode-preservation metadata classifies Transform, AlphaComposite, BlendModule, and CompositeModule as same-mode operations based on their output contracts; the transition planner also takes RemapPalette's P output and Color3DLut's exact target from their operation semantics. This permits direct mode reporting and safe prefix reuse for reviewed routes. Cross-mode lazy P/PA terminal palette metadata remains incomplete. `Image::from_dynamic` takes a mode-tagged raster; fallible `Image::from_dynamic_with_mode` remains at reviewed adapter boundaries. `Draw::new` accepts `Option<Mode>`; Python and JavaScript parse optional Pillow strings before calling it. `Image::mode()` remains the Pillow-compatible string projection. `PipelineOp` uses typed `Mode` inputs, including Merge's destination identity. Maintained `pool_cpu` operation paths select from exact `DynamicImage` variants; remaining mode-name formatting in CPU is for errors. GPU logical-mode helpers still consume Pillow spellings and have an explicit migration TODO. Keep destination modes and per-operation mode parameters where they express operation inputs, and use the exact mode tag on `DynamicImage` for materialized sources.
- At the baseline, `ColorMode` and `PixelMode` were separate mode-like enums. Maintained `PipelineOp` alpha mutations now use the exact raster `Mode`; the duplicate `PixelMode` type and its alias-collapsing mappings have been removed.
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

- Active-source dependency audit: searches of maintained Rust source found no imports or module declarations for `pool_cpu::legacy`, `super::legacy`, or `mod legacy`; maintained `compute/pool_cpu` and `compute/registry` contain no direct parallel feature gates, Rayon calls, or parallel iterator markers. Shared `color/grayscale` and `color/conversion` CMYK helpers stay serial, `ops/convert/nonstandard.rs` no longer changes eager receipt behavior based on the global `parallel` feature, and the maintained `ops/pil_resize` compatibility path now uses serial row loops regardless of Cargo features. Its former Rayon scheduler and threshold plumbing are reserved for the later `pool_parallel` owner. The audit does not prove every family is implemented, compiled, or verified.

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

The raster-owned mode identity is carried by the materialized DynamicImage variant itself. Do not infer it from channels, ColorType, buffer contents, or a separate string. image-slash-star owns its codec `ImageMode` transfer layout and its exact header-selected `image_slash_star::Mode` in `ImageInfo.raster_mode`; `ImageInfo.mode` remains the transfer layout. Keep pillow-rs `raster::Mode` as a separate operation-facing type. At encoded-input construction, the pillow-rs wrapper translates the cached header identity once and carries its own `Mode` while pixels remain lazy. The decode adapter applies that stored identity to the resulting `DynamicImage`. Lazy pipeline operations use the source mode and explicitly account for mode-changing operations before materialization.

Source-only progress: `raster::Mode` now owns `band_count()` for all 25
identities and `pillow_band_count()` for the 20 Pillow-named identities;
internal high-depth modes have native band counts but no Pillow band count.
GetChannel and GetData admission use the exact Mode count instead of inferring
it from image-slash-star's `ColorType`. PutPixel tuple shape, empty extrema,
statistics, Histogram, Entropy, GetColors, Split, and mode-gated histogram
traversal also read the mode-owned count. Histogram and Entropy now select
exhaustive exact-mode support and share the prepared Histogram child; Pillow
I/F and I;16 special rules remain native. CoreOnly LA16/RGB16/RGBA16 directly
bin native u16 samples across the full u16 domain, while RGB32F/RGBA32F directly
bin native f32 samples on `[0, 1]`; both keep 256 bins per band and allocate no
converted image or sample buffer. The u16 bin index matches the existing
rounded u16→u8 projection, and the f32 bin index matches its existing
normalized/clamped/rounded f32→u8 projection, applied directly to each sample.
Quantize selects exact L/P/RGB/RGBA strategies from Mode, and GetBBox uses
exact mode for alpha-only behavior. Native
transpose mirror uses the exact mode's band count for its byte stride, with
the four-byte I/F carrier represented explicitly. Byte Eval retains the
distinct Pillow-only count.
CoreOnly high-depth analysis readers have received source review; direct
operation receipts and mode × architecture parity remain pending. Split and
GetChannel return native scalar `ImageI16`/`ImageF32` bands for CoreOnly
high-depth modes; those call paths also require planned CPU parity and receipt
review.

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
| One, ImageLuma8 | Native one-byte reducer with mode-preserving output; One is an explicit Rust extension marked `NO_PILLOW_SUPPORT` because Pillow rejects Reduce for mode `1`. ImageLuma8 declares AVX2 and portable SSE2/NEON/wasm SIMD lane strategies. |
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

Source audit refinement (2026-11-02): the native typed u16/f32 reducer
now converts row coordinates to `usize` before flattening the sample
index, avoiding `u32` pixel-count overflow on valid large rasters.
Pillow 12.2 rejects Reduce for mode `1`; the maintained native One
byte route is explicitly recorded as a Rust-only extension. No checks
or runtime evidence ran.

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
| `compute/eager.rs`, `image/projection.rs`, `ops/module_fns.rs` | Eager RGB/CMYK GetProjection, LinearGradient, RadialGradient, and EffectMandelbrot use operation-owned CPU strategies and prepared receipts. Tiny-RGB eager ExtractBand now uses the prepared architecture executor for both serial CPU and compatibility SIMD requests, retaining the selected strategy in telemetry. The direct Blend and Merge wrappers now apply their mode-pair/band-layout contracts from the same declarations used by prepared execution while preserving eager error order. The generic receipt helper remains test-only. Continue auditing other direct eager routes and context injection. |
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
- This is the existing frozen manifest denominator, not yet the requested operation × exact Mode × declared-CPU-Architecture matrix. Preserve all existing case IDs, add the missing parity cases after source migration, and record the expanded manifest hash before the CPU parity run.
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

### Composite-family source progress: operation strategies source-reviewed; runtime parity pending

- The archived CPU implementation is
  `pool_cpu/legacy/ops/composite.rs`. `AlphaComposite` remains in the existing
  LA and RGBA strategies. Merge, BlendModule, and CompositeModule have
  operation-owned prepared plans and their registry entries point at those
  plans. The family `ops.rs` facade exposes support and preparation while
  `ops/{merge,blend,composite_module}.rs` keeps each contract beside its
  operation entry point. Exact mode strategies live under
  `composite/<mode>/default`, with shared native-sample helpers under
  `composite/helpers/`. These additions are source-only and unverified.
- `Merge` target contracts are declared in
  `compute/pool_cpu/ops/composite/ops/merge.rs` and applied by the typed Rust
  wrapper through `OperationId::Merge.support()`:
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
  owner also restores the portable wide routes; source comparison matches the archived block widths, scalar tails, empty/sub-block routing, and selection receipts; runtime parity remains pending. There is no active `pool_simd` pool.
- Source migration (2026-10-31): RGB's original planar SSSE3 and AArch64
  NEON interleavers live under `composite/rgb8/{ssse3,neon}` and remain guarded
  by the 16-pixel minimum, exact RGB target, L/P first band, remaining-L bands,
  equal dimensions, and required architecture. The archived portable
  `wide::u8x16` shuffle now has exact LA, RGB, RGBA, and CMYK mode entries for
  SSE2, NEON, and Wasm SIMD128. LA uses eight pixels per block; RGB, RGBA, and
  CMYK use four. RGB keeps the explicit SSSE3/NEON path first and uses the wide
  shuffle for smaller inputs. The SSSE3/NEON RGB fast paths retain their
  16-pixel guards and route smaller inputs through that architecture's
  portable wide kernel. The portable architecture declarations admit exact
  layouts with no complete block, so empty and tail-only inputs keep the
  archived `wide-shuffle` selection and receipts instead of falling back to
  Default. Every path preserves its exact output identity, scalar tail, and
  allocation/path/vector/tail telemetry. No Merge unit tests are being added; parity will cover each mode
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
  plan uses exact output identity and native byte, u16, and internal f32 sample
  paths; Pillow's I/F paths retain their archived four-byte carrier arithmetic.
  The public source-to-destination mode conversion remains at the wrapper
  boundary. Core-only high-depth formulas use native u16/f32 storage, are
  source-reviewed, carry `NO_PILLOW_SUPPORT`, and do not claim Pillow parity.
  One, L, P, LA, PA, RGB, HSV, YCbCr, RGBA, RGBa, RGBX, CMYK, I, and F declare
  SSE2, NEON, and Wasm SIMD128 masked-byte paths through their exact-mode Paste
  architecture owners, with same-mode Default fallback. I, F, and RGBa keep all
  four native carrier bytes. Their guard
  requires exact source/destination mode, valid native lengths, and a matching
  mask shape and selector. The operation-level support view exposes this
  input declaration, and the direct typed Rust wrapper reads the same declared
  mask sets, luma-versus-alpha selector, and size error while preserving its
  size-before-mask rejection order. Mask samples are borrowed from native
  storage; mode `1` is treated as a binary mask. Source comparison found the
  archived SIMD Composite plan also admitted `RGBa`, `I`, and `F` as raw
  four-byte layouts. Their exact-mode declarations use the existing
  native-byte vector kernel and reconstruct the original mode. Source-only
  and unvalidated.
  Parity must cover every exact mode, mask selector, architecture, block/tail
  boundary, and fallback.
- `prepare_composite_pipeline` materializes the mask before image2, matching
  the archived executor's lazy-input error order. The direct wrapper checks
  source/mask dimensions before validating the mask mode, then resolves
  image2's output mode, converts the source at the wrapper boundary, and
  retains image2's P/PA palette metadata. These caller and error-order paths
  have been compared with the `3f9dfd403` baseline; parity remains pending.
- Source comparison found that archived Pillow I/F Composite operates on all
  four raw carrier bytes, rather than interpolating an integer or float sample.
  The exact `ImageI32` and `ImageF32` owners now use the four-byte Composite
  helper and reconstruct their exact DynamicImage variants at output. The
  dedicated numeric I32 path was removed; float-sample execution remains only
  for the internal RGB32F/RGBA32F extensions. This source correction is
  uncompiled and unvalidated; include I/F raw-byte edge cases in CPU parity.
- Core-only high-depth Composite paths are source-reviewed independently of
  Pillow parity. I;16 variants retain native u16 samples and exact I;16 output
  identity; internal LA16/RGB16/RGBA16 use their native u16 channel counts;
  internal RGB32F/RGBA32F use native f32 samples. The shared output builders
  reconstruct each declared variant, and every internal mode owner carries a
  nearby `NO_PILLOW_SUPPORT` marker. The exact mask identities remain the
  operation's declared One/L/LA/RGBA/RGBa set. This review establishes the
  declared storage and routing, not runtime parity; no validation has run.
- `AlphaComposite` remains restricted to exact LA and RGBA inputs, preserves
  Pillow's fixed-point pixel arithmetic and existing ignored-offset behavior,
  and has maintained SSE2/NEON/Wasm SIMD128 routes under exact LA/RGBA
  architecture folders. Those small wrappers reuse one shared wide kernel in
  the family helpers. Its full-frame RGBA `AlphaComposite+Mirror` fusion is
  wired through prepared LA/RGBA and Mirror selections in CPU batch execution,
  retaining each operation's architecture receipt. Its eligibility and
  fallback cases remain in the operation-by-mode-by-architecture parity
  matrix. Do not fold the fusion into CompositeModule.
- The RGBA fusion preparer now declines unequal source/destination dimensions,
  matching the archived batch fusion guard. This avoids invoking the fused
  kernel for a request outside its same-size contract; this source change is
  uncompiled and unvalidated.
- Remaining dispatch comparison: the archived Merge adapter ran its portable
  interleave path for sub-block and empty layouts, finishing with scalar
  samples. The maintained portable architecture guard now admits those exact
  layouts too; RGB's SSSE3/NEON specializations retain their 16-pixel guard
  and fall back to the same architecture's portable wide interleaver below
  that threshold. AlphaComposite request facts now retain the ignored source
  and destination offsets; its
  vector guard matches the archived zero-offset and empty-or-at-least-eight-
  pixel admission. The shared vector entry records the archived
  `scalar-control` receipt for empty images and vector-block/scalar-tail counts
  for nonempty inputs. These source changes are uncompiled and unvalidated;
  pixel output continues to preserve the existing ignored-offset behavior.
- Composite's non-Default byte route now composes the corresponding exact-mode
  Paste strategy: `One`, `L`, `P`, `LA`, `PA`, `RGB`, `HSV`, `YCbCr`, `RGBA`,
  `RGBa`, `RGBX`, `CMYK`, `I`, and `F` each call their own Paste `ops.rs`
  dispatcher. `BytePlan` accepts either the owned inputs used by Paste or
  borrowed source/mask images used by Composite, so this path does not clone
  images or add dynamic dispatch in the kernel. Composite Default continues
  through its serial helper. Its byte output setup preserves the archived L
  clone versus non-L zero-and-copy behavior and copy receipts; the empty
  overlap records the archived `vector` path. The retired generic Composite
  vector helper and duplicate per-mode Default Composite wrappers were removed
  after their call sites moved. Source review compared the archived dispatcher
  mode set and raw I/F/RGBa layouts with the maintained owners. The shared
  masked kernel follows the archived 16-byte blocks, 8-byte blocks, and
  padded short-vector tails. Scalar-tail receipts follow the archived mode and
  mask route: binary rows count active bytes left after 16-byte blocks; generic
  fractional rows count active bytes left after 8-byte blocks; dedicated L,
  LA, PA, RGB, and HSV L-mask rows omit padded-tail scalar counts, while the L
  NEON row reports its actual scalar remainder. Composite support declarations expose the exact
  L-mask NEON row guards for L, LA, RGB, HSV, RGBA, and CMYK before each mode's
  generic NEON fallback. RGB and HSV share the same three-channel row kernel
  under paste/helpers/kernels; the mode owners supply their exact identity.
  Shared Paste mode owners invoke specialized rows only when the selected
  Paste or Composite guard matches the exact mode, channel count, and width
  threshold; Composite's guard measures the source/image2 top-left overlap
  because image2 is a differently sized destination canvas. L, LA, RGBA,
  CMYK, and RGB/HSV L-mask NEON formulas, block/tail handling, and receipts now
  match the archive by source comparison. Still pending: compare the other modes'
  binary, alpha, and premultiplied mask routes,
  specialized architecture paths, and exact block/tail/path receipts against
  the archive. AlphaComposite callers and RGBA Mirror fusion conditions are
  source-reviewed above. This remains uncompiled and unvalidated.
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
  bytewise operation. The 12 direct byte-binary wrappers and three logical
  wrappers now pass their exact `OperationId` to the support declaration for
  source and secondary-mode admission; the duplicate channel-width class map
  is removed. Binary wrappers still resolve both input identities before
  rejection, logical wrappers still reject the first mode before reading the
  second, and overlap clipping is unchanged. Unary Constant, Offset, Duplicate,
  and Invert remain lazy: their Rust wrappers queue the matching `PipelineOp`,
  `Image::push_op` keeps the declared L transition or exact-mode preservation,
  and the shared registered `prepare_pipeline` route selects the corresponding
  `OperationId` support contract at materialization. Python and JavaScript
  wrappers delegate these operations to the typed Rust API. This retains the
  existing deferred execution/error boundary without duplicating mode checks.
  Their public Rust/Python caller and registry paths are source-reviewed
  against the baseline. Binary wrappers resolve both exact input identities
  before pair rejection; logical wrappers reject the source mode before
  resolving the secondary identity. The Chops owner validates the exact source
  contract, materializes the secondary only during preparation, selects the
  declared architecture or same-mode Default fallback, and dispatches the
  exact mode owner. P/PA Chops preserve primary palette metadata through the
  indexed wrapper path; Constant plans L, while other unary operations retain
  their exact output identity. Compatibility `Backend::Simd` routes through
  `CpuPool` with the centrally detected Architecture. Owned Add/Sub, Invert,
  and Multiply→Screen batch routes and their receipt/fallback paths have been
  source-traced. No formula or error-order discrepancy was found. Chops source
  review is complete; runtime parity and telemetry evidence remain pending.
  This source remains unvalidated.
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
  its physical channels, active channels, and float32-exact shift shortcuts;
  the family helper receives those facts and never infers a mode. It returns
  the same typed raster, while unavailable or ineligible architectures retain
  that mode's Default strategy. The L Default owner retains exact binary
  right-shift shortcuts, selected after converting the factor to float32.
  Every other byte sample follows the same Pillow blend expression in Default
  and architecture paths. This source change is uncompiled and unverified.
  Source comparison covers all 11 Pillow byte identities. `ImageEnhance.Brightness`
  copies only an uppercase `A` band into its degenerate image before blending.
  Therefore LA and RGBA preserve alpha; La and RGBa blend their lowercase `a`
  samples; RGBX blends its X sample; RGB, HSV, YCbCr, and LAB blend their three
  stored samples; and CMYK blends all four samples. The old shared-carrier route
  treated La like LA and RGBa/RGBX like RGBA. Their exact-mode Default owners
  now use the Pillow band-name behavior instead of inheriting those collapsed
  carrier choices. See [Pillow ImageEnhance](https://github.com/python-pillow/Pillow/blob/main/src/PIL/ImageEnhance.py),
  [Pillow ImageMode](https://github.com/python-pillow/Pillow/blob/main/src/PIL/ImageMode.py),
  and [Pillow Blend.c](https://github.com/python-pillow/Pillow/blob/main/src/libImaging/Blend.c).
  The maintained mode owners bind results to exact `DynamicImage` identities.
  The byte Default and architecture helpers now convert the factor to float32
  and share Pillow's `base + alpha * (sample - base)` arithmetic, clipping
  extrapolation and truncating the result. The architecture LUT is generated
  from that same helper instead of a milli-factor approximation. The L/LA/RGB
  binary shift fast paths are used only when the float32 factor equals the
  exact power-of-two value. This aligns source arithmetic with Pillow's
  `Blend.c`; factor-boundary outcomes and selected implementations still need
  CPU parity evidence. In particular, sample 10 with factor
  `0.09999999999999999` must produce 1 after Pillow float32 conversion; include
  this case on Default and every declared vector architecture with
  selected/fallback receipts. See [Pillow 12.2.0 ImageEnhance](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageEnhance.py),
  [Pillow 12.2.0 Blend.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Blend.c).
  The public route
  is source-traced: Python Brightness delegates to typed Rust
  `Image::enhance_brightness`; it validates the exact image mode and queues a
  `PipelineOp`; `Image::push_op` retains the image backend; CPU pipeline routing
  creates the injected `ExecutionContext`; registry preparation passes it to
  this family owner; and `prepare_mode` matches the exact `DynamicImage` variant
  before the mode owner selects its declared architecture or Default fallback.
  The exact owner wraps the result in that same DynamicImage variant. Source
  review verified that each of the seven declared vector architectures has a
  matching exact-mode execution arm; the other accepted byte modes declare
  Default only. The shared `ModeSupport::select` path applies the injected
  architecture policy, build and hardware availability, and input guards,
  then records both the selected implementation and the first fallback reason.
  Runtime validation remains pending for the operation × mode × architecture
  parity matrix, including selection and fallback receipts; see the Brightness
  TODO in `compute/pool_cpu/ops/enhance/ops.rs`.
  The typed Brightness wrapper's three rejections also match the archived
  `validate_mode(self, false)` boundary exactly: One, P, and PA return
  `image has wrong mode`; the other 22 exact identities queue execution.
  Numeric-mode archive comparison found material route differences: baseline
  decode maps I/F payloads to an RGBA byte carrier, whose Brightness leaf
  scales the first three stored bytes and preserves byte four; current I/F
  owners scale native i32/f32 samples. Baseline I;16 materializes as L16 and
  falls through an 8-bit RGB conversion; current I;16 owners scale typed u16
  samples and preserve exact mode identity. Pillow's ImageEnhance Brightness
  delegates to `Image.blend`, and Pillow's blend implementation rejects
  non-u8 sample storage. These native I/F, I;16-family, and internal high-depth
  strategies are now explicitly declared `CoreOnly` with `NO_PILLOW_SUPPORT`
  comments in their mode owners. Their behavior is a Rust extension and must
  be reported separately from Pillow parity; this does not claim equivalence
  to the archived carrier/conversion side effects.
  Coverage gap: the checked-in parity manifest currently declares only five
  Brightness mode requirements (L, LA, RGB, RGBA, CMYK), not the requested
  operation × exact-mode × declared-CPU-architecture matrix. Preserve those
  cases and expand after source review; report Pillow-defined parity separately
  from CoreOnly extension coverage, then freeze and record both denominators
  before execution.
  CPU registry preparation points to this owner, and Backend::Simd compatibility
  requests enter CPU architecture selection; no separate adapter registration
  is active. The Brightness and Sharpness public wrappers now read mode
  acceptance and rejection errors from their `OperationId` support contracts
  instead of maintaining separate rejection lists. Source integration has not
  been compiled or parity-verified.

### Remaining enhancement source review: in progress and unvalidated

- `Image::enhance_contrast` creates a lazy operation for L, LA, RGB, RGBA,
  RGBX, CMYK, HSV, YCbCr, and LAB. The eight non-LAB byte owners use their
  exact-mode prepared strategy; LAB prepares its Convert-to-L, rounded mean,
  L-to-RGB, ConvertLab, and Blend composition. Other source modes enter the
  `contrast_degenerate()`/Blend wrapper composition, which can preserve
  mode-specific conversion errors. `RGBa` returns
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
  prepared operation contract classifies all 25 identities: eight queued byte
  modes use native Default strategies and LAB uses its prepared conversion and
  blend composition. La and RGBa report their exact conversion/guard
  rejections. One, P, PA, I, F, and all four I;16 identities construct the
  existing same-mode base, then Blend rejects them with `image has wrong mode`;
  PA's route also extracts and reapplies its uppercase A band before Blend.
  The five internal high-depth identities reject the L-to-source base
  conversion with that same error. The wrapper retains its conversion-first
  error timing; no extra Contrast pixel strategy is warranted.
  `CONTRAST_SUPPORT` now uses `OperationSupport::routed` for all 25 modes: the
  nine prepared admissions reuse the existing classifier, while conversion
  fallbacks expose their exact L intermediate, ordered child operations,
  output identity, and rejection stage. PA explicitly records ExtractBand and
  PutAlphaData before Blend's wrong-mode rejection. This source mapping still
  needs CPU parity and architecture/fallback receipts.
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
  any pinned source backend on the final blend pipeline. Source comparison of
  the queued RGBX and CMYK routes against their former conversion/blend
  fallback found matching bases: L→RGBX supplies X=255, while L→CMYK supplies
  (0,0,0,255−L); the native CMYK strategy decodes the exact CMYK bytes to
  RGB/luma before constructing that same CMYK base. The archived adapter is
  inactive; its zero-factor RGBA vector path retains the archived
  integer-luma and alpha-shuffle kernel rather than routing through the generic
  eight-lane float blend. Caller trace is now explicit: Python `Color` chooses
  L/LA from its band names at the host boundary, calls Rust `color_degenerate`
  for non-L/LA inputs, then routes `enhance` through public Blend except for
  its RGB factor-0/1 shortcuts. Direct Rust `Image::enhance_color` instead
  selects the six native prepared strategies and uses conversion plus
  BlendModule for the other 19 modes. Reconcile each fallback's base mode,
  conversion/error order, and output identity across both entry paths; the
  six-mode prepared declaration alone is not the complete public Color contract.
  `raster::Mode::uppercase_a_band_index()` is the exhaustive owner of the
  uppercase-A band rule: band 1 for LA, PA, and LAB; band 3 for RGBA; and the
  corresponding internal LA16/RGBA16/RGBA32F identities. Lowercase `a` in
  La/RGBa does not select an LA base. ColorSaturation derives its L/LA
  intermediate from that typed mode property and exposes it through the route
  descriptor consumed by `Image::color_degenerate`. The remaining 19 exact
  modes have source-traced
  outcomes through `color_degenerate()`, Convert, and BlendModule:
  - YCbCr (`L -> YCbCr`), HSV (`L -> HSV`), and LAB (`LA -> LAB`) complete the
    base conversion, pass BlendModule's compatible three-channel contract, and
    preserve their exact output identity.
  - One, P, PA, I, F, and all four I;16 identities complete the base conversion
    back to their exact mode, then BlendModule rejects the primary mode with
    `image has wrong mode` before checking dimensions. P/PA base conversion
    produces wrapper-owned palette output; Python Color also applies its
    existing transparency-info handling during construction.
  - La fails on source-to-L conversion with `conversion from La to L not
    supported`. RGBa converts to L, then the wrapper's explicit guard returns
    `conversion from L to RGBa not supported` without attempting a second
    conversion. Internal LA16, RGB16, RGBA16, RGB32F, and RGBA32F convert to
    the L/LA intermediate, then reject conversion back to the internal target
    with `image has wrong mode`.
  Python `Color` constructs this base eagerly, uses public Blend for all
  non-shortcut enhancements, and retains its RGB factor-0/1 shortcuts. Direct
  Rust `Image::enhance_color` queues the six native strategies and synchronously
  builds the conversion base before composing Blend for the other modes. The
  operation now exposes all 25 routes through `OperationModeRoute`, including
  the exact intermediate mode, child operations, result/error stage, and
  prepared admission. Both `Image::enhance_color` and `Image::color_degenerate`
  consume this declaration. The full per-mode route mapping and remaining
  validation scope are recorded below; Python's eager-base/public-Blend path
  remains a distinct host route. This source mapping has not been
  runtime-validated.
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
  Default implementation; its operation/mode/architecture contract is
  source-reviewed above. ColorSaturation has a complete 25-mode
  `OperationModeRoute` declaration at its operation entry: six exact native
  prepared strategies, three successful composed routes, nine composed
  Convert+Blend rejections, the La source-conversion rejection, the RGBa
  wrapper-guard rejection, and five internal high-depth result-conversion
  rejections. Each route declares its exact L or LA intermediate, ordered
  child operation IDs, output identity, route scope, and prepared admission.
  `Image::enhance_color` selects prepared versus composed behavior from this
  declaration, while `Image::color_degenerate` reads the declared intermediate
  from the same route. The composed successes are YCbCr and HSV through L, and
  LAB through LA. One, P, PA, I, F, and the four I;16 identities complete the
  conversion round trip before Blend rejects them; P and PA expose the Convert
  child's palette dependency. La fails converting to L. RGBa constructs its
  source-to-L request, then the explicit wrapper guard returns the existing
  `conversion from L to RGBa not supported` error. Internal LA16, RGB16,
  RGBA16, RGB32F, and RGBA32F reach the existing invalid result-conversion
  error. Python `ImageEnhance.Color` snapshots `color_degenerate()` and calls
  public Blend, with its existing RGB factor-zero/factor-one shortcuts; the
  direct Rust `Image::enhance_color` path remains separate. Source tracing
  found that the eager base can lose the source pipeline's backend pin, so
  `module_fns::blend` now inherits the first operand's explicit backend or the
  second operand's when the first has none. Include both host and direct Rust
  routes, metadata behavior, and backend receipts in CPU parity. Archived
  AVX+FMA/NEON kernels
  remain under the six native mode owners. The independent SIMD registration
  is inactive and its archive is reference-only. These routes are source
  reviewed but not runtime-validated; the ColorSaturation parity TODO remains
  open.

### Chops source mapping: source-reviewed and unvalidated

- Offset now has source-only SSE2, NEON, and Wasm SIMD128 declarations for the
  archived byte identities (`1`, `L`, `P`, `LA`, `PA`, `RGB`, `HSV`, `YCbCr`,
  `RGBA`, `CMYK`, `RGBa`, `RGBX`, `I`, and `F`) and for each exact I;16
  identity. The byte guard retains the archived empty-image or 16-total-byte
  admission and its checked-u32 overflow boundary. The I;16 guard checks the
  exact typed byte length and checked-u32 overflow boundary, admitting empty
  or at-least-16-byte buffers. The archived shape predicate also admitted
  smaller nonempty I;16 frames, but its kernel cannot produce a result for
  those frames; the maintained guard now selects the exact-mode Default
  strategy for them. Each ineligible architecture request retains the
  same-mode Default fallback. The exact-mode declarations expose SSE2, NEON,
  and Wasm SIMD128 routes for the 12 archived byte modes, I, F, and all four
  I;16 identities. `La`, LAB, and the five internal high-depth identities
  remain Default-only. The typed byte kernels receive buffers from the exact
  mode/architecture owner and return the same buffer type; channel width and
  output identity are not selected in the shared helper. Default Offset also
  reconstructs each exact `DynamicImage` variant in its own mode folder, and
  raster transforms no longer own a mode-matching Offset route. The moved byte
  and I;16 kernels preserve Pillow's byte-oriented I;16 behavior: rotate the
  first `width` bytes of each `2 * width` row, zero-fill the rest, and byte-swap
  only the exact `I;16B` identity at the typed boundary. Source review compared
  the kernel arithmetic, typed route, architecture declaration/dispatch map,
  context flow, selection/path receipts, and public pipeline/batch route with
  the archive. The I;16 sub-16-byte fallback corrects the archived mismatch
  between its broad shape admission and a kernel that cannot produce output
  without a complete 16-byte block.
  P/PA Offset retains indexed samples and palette metadata through
  materialization, then selects the original exact-mode strategy. This source
  mapping remains uncompiled and unvalidated; parity is pending.
- Archived native-byte modes `1/L/P`, `LA/PA`, `RGB/YCbCr/HSV`, and
  `RGBA/CMYK/RGBa/RGBX` declare SSE2, NEON, and Wasm SIMD128 candidates for
  the archived vector operations; mode `1` also declares its three
  truth-based logical vector operations. `La` has no archived SIMD Chops
  strategy and now selects Default. LAB binary Chops use the native three-byte
  Default path; the archived vector layout did not include LAB. Add/Sub retain
  their finite parameter, equal-dimension, and minimum-row-width guard. Other vector
  operations require equal dimensions. Incompatible shapes or unavailable
  architectures select the same mode's serial Default implementation, which
  retains overlap clipping. All kernels read and write the selected mode's
  native bytes and reconstruct its exact mode without conversion.
- The archived Multiply/Screen widening and divide-by-255 arithmetic,
  min/max/difference/modulo byte lanes, Overlay/HardLight divide-by-127 LUT
  formulas, SoftLight integer formula, and mode-`1` canonical truth results
  have been moved into the family-owned vector helper. The compatibility route
  is source-traced: `Backend::Simd` enters `CpuPool` with the centrally
  selected Architecture; no active `pool_simd` backend remains registered.
  Exact-mode dispatch and runtime fallback still require verification.
- Source comparison now covers the shared Default and vector Add/Sub,
  Multiply/Screen, bytewise, LUT, and logical formulas against their archived
  CPU/SIMD implementations; no arithmetic discrepancy was found. The typed
  public wrapper preserves the baseline `1/L/P`, `LA/La/PA`, `RGB/YCbCr/HSV`,
  and `RGBA/CMYK/RGBa/RGBX` groups, and declares LAB separately as CoreOnly. The
  prepared Multiply→Screen route retains the archive's exact L/LA/RGB/RGBA
  modes and requires matching dimensions, secondary identity, and selected
  architecture. These findings are source-only. Source review for exact-mode
  dispatch, direct/eager/compatibility callers, and public error order is
  complete; runtime dispatch, fallback receipts, compilation, and parity remain
  unverified.
- LAB's native three-byte binary extension is now explicit in the support
  declaration: LAB-primary binary operations are CoreOnly, and `ChopsBinaryModes`
  identifies LAB secondary operands and their Rust-only reason. The vector
  guards leave both `La` and LAB secondary operands on the primary mode's
  Default path, matching archived SIMD admission while preserving the existing
  serial compatibility groups. This correction is source-only and uncompiled.
- The exact-mode architecture declarations differ by Chops operation. Add,
  Subtract, Multiply, Screen, Darker, Lighter, Difference, Overlay, HardLight,
  SoftLight, AddModulo, and SubtractModulo have vector candidates for the 12
  archived byte identities `1/L/P`, `LA/PA`, `RGB/YCbCr/HSV`, and
  `RGBA/CMYK/RGBa/RGBX`; LogicalAnd/Or/Xor have candidates only for `1`.
  Chops Invert has candidates only for `1/L/P`, `LA/PA`, `RGB`, and
  `RGBA/CMYK`, with the Chops path inverting all stored bytes. Offset has
  candidates for the 12 byte identities plus `I`, `F`, `I;16`, `I;16L`,
  `I;16B`, and `I;16N`. `La` and `LAB` retain serial Default binary/Offset
  strategies where declared; internal high-depth modes retain their declared
  Default or CoreOnly strategies. These operation-specific maps were checked
  against the archived layout predicates and the maintained declarations;
  runtime dispatch and fallback receipts remain unverified.
- Binary Default and architecture helpers consume typed primary and secondary
  `ImageBuffer<P, Vec<u8>>` values. Each exact-primary-mode owner accepts a
  secondary only from its declared native channel-layout group: `1/L/P`,
  `LA/La/PA`, `RGB/YCbCr/LAB/HSV`, or `RGBA/CMYK/RGBa/RGBX`. It then wraps the
  result in the primary mode. The helper derives channel count from
  `Pixel::CHANNEL_COUNT`; no operation kernel accepts Pillow `Mode` or
  reconstructs `DynamicImage`. The owned Add/Sub route applies the same typed
  boundary, including compatible cross-mode operands in accelerated and
  in-place paths. Its in-place eligibility reads dense row width rather than
  keeping a second mode-to-channel table. These moves preserve overlap
  clipping, same-dimension vector checks, affine arithmetic, row traversal,
  and primary output identity. Source review also found and restored missing
  typed `execute` orchestrators for L/LA/RGB/RGBA, including their Offset
  architecture routes. The changes remain uncompiled and unvalidated.
- The archived SIMD batch admitted in-place Chops only for Add/Sub, and its
  affine path required empty storage or at least eight bytes per row. The CPU
  batch now prepares one `ChopsExecutionPlan` from the mode support declaration
  and reuses an owned prior result only when that selected architecture plan
  meets the archived threshold. Unit Add/Sub uses the 16-byte saturating
  in-place kernel only with a complete vector per row; 8–15-byte rows retain
  the archived eight-lane affine path, narrow rows use the allocating selected
  strategy, and empty images are a no-op. The route records the selected
  architecture without selecting again. Successful helpers record `vector`
  or `native-copy` plus their block/tail counts. If an admitted in-place
  architecture helper fails, the batch now records the archived `unsupported`
  receipt before finishing the operation. Eligibility, fallback, and telemetry
  paths are source-traced; no runtime evidence exists.
- Chops Add/Sub support facts retain the original `f64` scale and offset bits
  for the archived finite-value and exact `(1, 0)` architecture guard. The
  executable request still narrows to `f32` for Pillow's arithmetic. Thus a
  finite `f64` that overflows when narrowed remains eligible for the archived
  affine vector path, while non-finite original parameters select Default.
  Architecture admission also checks exact primary and secondary native byte
  lengths; scaled nonempty work requires at least eight bytes per row, while
  empty and exact unit-parameter work follows the archived empty-storage and
  row traversal rules. This guard and narrowing boundary are source-reviewed
  only.
- The archived Multiply→Screen scalar and vector formulas now have a prepared
  native-mode fusion for L/LA/RGB/RGBA. CPU batch execution admits it only for
  adjacent operations sharing the same secondary `Image`, matching mode and
  dimensions, and matching Multiply/Screen architecture selections. It keeps
  the intermediate multiply truncation and performs one output traversal.
  Prepared execution now dispatches through the exact mode's Default or named
  architecture entry point; the shared arithmetic remains in
  `chops/helpers/fusion.rs` without an Architecture match. Explicit
  admission validates exact native byte lengths for all three inputs and
  preserves the archived zero-height/nonzero-width empty fusion. Allocating
  vector execution also retains its selected architecture telemetry. If
  admission returns `None` because a source, mode, shape, or architecture
  condition does not match, the batch continues through the original Multiply
  and Screen operations separately; materialization errors propagate at the
  first operation that needs the secondary image. On success, each fused
  operation receives the selected architecture, actual path, vector block
  count, and scalar tail count. These fallback and receipt paths are
  source-traced against the archived CPU/SIMD callers, but remain unverified
  at runtime. Both serial and architecture helpers now receive the matching
  typed buffers and return a typed image; the exact-mode owner wraps that
  output. The shared fusion helper no longer inspects `DynamicImage` or accepts
  `Mode`.
- The public Rust Chops adapter checks `raster::Mode` from known wrapper
  identity, materializing the source only when that typed identity is unknown;
  this removes its last Pillow-string mode comparison. Internal high-depth
  unary support is classified as CoreOnly at the operation descriptor.
- Indexed Chops outputs keep the primary image's palette metadata. Pillow's
  `libImaging/Chops.c` returns a new P/PA core, then `ImageChops` wraps it with
  `Image._new`, which copies the source palette for indexed results. Removed a
  mistaken special case that cleared palette and per-entry alpha metadata from
  P/PA Chops pipelines. This metadata correction is source-reviewed and still
  needs CPU parity evidence.
- The archived SIMD Chops Invert source accepts empty native layouts and
  returns the input with a `native-copy` receipt. Its maintained shared kernel
  has the same empty path, but the initial nonempty-only strategy guard made
  that branch unreachable. A separate exact-byte guard now permits zero
  dimensions for ChopsInvert while still rejecting non-exact storage; Eval
  retains the nonempty guard because both identity and mapped vector kernels
  reject a zero row stride. SSE2, NEON, and Wasm SIMD128 compile-time feature
  declarations and the same-mode Default fallback are unchanged. Each
  byte-vector mode now exposes its declared architecture folders; mode `ops.rs`
  selects the named entry point, while the identical portable vector algorithm
  remains shared in `chops/helpers/vector.rs`. That helper no longer receives
  or checks Architecture. This is source-only evidence; parity and telemetry
  receipts remain pending.

| Preserved source/function | Maintained destination and disposition |
| --- | --- |
| `pool_cpu/legacy/ops/chops.rs::op_chops_add` through `op_chops_subtract_modulo` | `chops/helpers/binary.rs` preserves native-byte formulas and row clipping; exact mode support and secondary mode compatibility live in `chops/ops.rs` and each mode's `ops.rs`. Add/Sub architecture guards retain the original `f64` parameters before the `f32` arithmetic narrowing, matching archived finite checks and the exact `(1, 0)` fast-path test. |
| `pool_cpu/legacy/ops/chops.rs::op_chops_logical_and` through `op_chops_logical_xor` | `chops/helpers/binary.rs` uses truth semantics on raw mode-1 bytes and emits canonical 0/255 bytes. |
| `pool_cpu/legacy/mod.rs::fused_multiply_screen` and `pool_cpu/legacy/pool_simd/ops/adapters/chops/kernels/fused_multiply_screen.rs` | `chops/helpers/fusion.rs` preserves serial and vector arithmetic for exact L/LA/RGB/RGBA. `chops/ops.rs` prepares both mode/architecture declarations and dispatches the selected fusion through each mode's Default or named architecture entry point; `batch_execution.rs` admits adjacent operations with the same operand and falls through to the original operations when fusion is ineligible. Compatibility routing is source-traced; parity/runtime verification remains pending. |
| `pool_cpu/legacy/ops/chops.rs::op_chops_constant`, `op_chops_offset`, `op_chops_duplicate` | `chops/helpers/unary.rs` keeps L output for Constant and same-mode copying for Duplicate. Each exact mode's `default/ops.rs` owns Offset's Default result variant and calls the typed shared helper in `chops/helpers/offset/default.rs`; byte and I;16 architecture owners pass typed buffers into `chops/helpers/offset/{byte,luma16}.rs` and wrap the result in that same mode. Their guards retain archived checked-u32 limits. The I;16 guard admits empty or at-least-16-byte buffers; smaller non-empty inputs select same-mode Default because the native kernel needs a full 16-byte block. Mode/architecture support, fallback, and receipt paths are source-reviewed; runtime parity remains pending. |
| `pool_cpu/legacy/pool_simd/ops/adapters/chops.rs::{supports_constant_for_image,supports_constant_for_shape,simd_constant}` | Intentionally retired as an architecture strategy: the SIMD adapter only allocated the final repeated-byte L buffer and reported synthetic vector counters; it executed no vector kernel. `chops/helpers/unary.rs::constant` retains the same output and fill behavior for every source mode, including empty images. All exact-mode ChopsConstant declarations route to Default; no architecture folder is added for this non-kernel path. |
| `pool_cpu/legacy/pool_simd/ops/adapters/chops.rs::{supports_in_place,execute_in_place}` and `kernels.rs::native_chops_affine_in_place` | The archive's batch calls the in-place adapter only after `supports_in_place`, which admits Add/Sub and requires empty native bytes or rows at least eight bytes wide. `chops/helpers/vector.rs::execute_in_place` preserves the affine eight-lane row traversal and the 16-byte saturating path for unit parameters when each row contains a full vector. `ChopsExecutionPlan` carries the already selected architecture into the owned-intermediate path in `pool_cpu/batch_execution.rs`; narrow unit rows keep the allocating vector route. The old bytewise in-place arms were not admitted by `supports_in_place` and are retired as unreachable from archived batch execution. Source admission and receipt handling are reviewed; runtime parity remains pending. |
| `pool_cpu/legacy/ops/chops.rs::op_chops_invert` | Each exact mode's `default/ops.rs` destructures and reconstructs its own `DynamicImage` variant. `chops/helpers/unary.rs` complements typed u8/u16 storage or all stored f32 bits, matching the current exact-mode raw-byte inversion without channel-count-based output selection. Internal high-depth identities are declared CoreOnly and their owners carry `NO_PILLOW_SUPPORT` comments. The archived byte-vector kernel is shared from `pool_cpu/helpers/bytes/native_invert.rs` by exact-mode Chops and ImageOps architecture strategies. No `Mode` argument or mode-based output constructor remains in the Chops Invert helper. |
| `pool_cpu/legacy/pool_simd/ops/adapters/chops/kernels.rs` | The exact `1/L/P`, `LA/PA`, `RGB/YCbCr/HSV`, and `RGBA/CMYK/RGBa/RGBX` mode owners expose SSE2, NEON, and Wasm SIMD128 entry points under `<mode>/<arch>/mod.rs`; `La` and LAB have no archived vector route. Those paths route into the shared portable formulas in `chops/helpers/vector.rs`. Operation support supplies exact mode, pair, dimension, and parameter guards, with same-mode Default fallback. |
| `pool_cpu/legacy/pool_simd/ops/adapters/effects.rs::{native_invert,simd_invert_chops}` | `chops/<mode>/{sse2,neon,wasm_simd128}/mod.rs` exposes the exact-mode architecture entry point for archived `1/L/P`, `LA/PA`, `RGB`, and `RGBA/CMYK` layouts. Each entry point reuses `chops/helpers/vector.rs`; mode `ops.rs` selects it from the injected Architecture. Channels including alpha are inverted. Empty exact layouts return the source with `native-copy`; the maintained exact-byte guard now admits them, with the same-mode serial Default as fallback. |

### Grayscale source progress snapshot (superseded by the current review below)

These implementation notes record an earlier review state. The current status
and remaining caller work are summarized in the later grayscale review section
and the CPU plan.

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

- `put/ops.rs` exposes `EXTRACT_BAND_SUPPORT` and concrete prepared strategies for all 25 raster identities. Each result is `ImageLuma8`. This describes the existing internal PipelineOp, whose defensive index behavior remains available to direct internal callers. The public `Image::getchannel` wrapper owns selector validation and rejects I/F before materialization or index validation. Pillow `Bands.c` returns a source-mode copy for any one-band UINT8 image; maintained routes preserve One and P identity (including P palette), while I;16 identity copies are explicitly CoreOnly. Other multiband byte modes produce L, and native high-depth multiband extensions retain their typed I;16/F output modes. Its remaining string-based metadata boundary is migration debt, not a new typed-boundary completion claim.
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
| `legacy/ops/put.rs::op_put_pixel` | Typed mode `default/put_pixel_impl.rs` owners compose `put/helpers/put_pixel.rs::write`; compact LA/La/PA and RGB/YCbCr/LAB/HSV owners use `write_with_hidden_lanes` to update omitted Pillow bytes. Native sample placement, one clone, and exact mode identity remain. |
| `Image::putpixel_mode_scalar` for I/F | `put/ops.rs::PUT_PIXEL_VALUE_AT_SUPPORT` classifies the exact mode and binds a CPU Default selection. `put/i32/default/put_pixel_impl.rs` and `put/f32/default/put_pixel_impl.rs` mutate the exact native four-byte sample in place through the shared checked byte-range helper; the wrapper records the selected strategy. |
| `Image::putpixel_l16` for I;16 variants | The same operation contract selects exact `i16`, `i16l`, `i16b`, or `i16n` Default owners. They write the existing native u16 sample in place without converting modes; the wrapper retains modulo-65536 normalization and records the selection. Source-only; unverified. |
| `legacy/ops/put.rs::{apply_put_pixel_batch,op_put_pixel_batch}` | Default owners compose the ordered `write_batch` helper with concrete pixel constructors; compact LA/La/PA and RGB/YCbCr/LAB/HSV owners use `write_batch_with_hidden_lanes`. Original descriptor/bounds errors and last-write-wins order remain. |
| Archived vector `native_putpixel_layout` and shape/image admission | Linked mode and architecture declarations in `put/ops.rs`, mode `ops.rs`, and `put/helpers/put_pixel_strategy.rs`; typed source identity replaces string/layout guesses. |
| Archived `simd_put_pixel_native` | `put/helpers/put_pixel_vector.rs::single`, reached through exact mode/architecture implementation owners; compact LA/PA and RGB/YCbCr/LAB/HSV owners use `single_with_hidden_lanes` to restore mode-owned omitted-byte payloads after vector finalization. Original masked store and finalization are preserved. |
| Archived nonblending `draw_line_vector_block` branch | `put/helpers/masked_store.rs`; compile-time native channel width removes the unused blending decision from PutPixel without changing source-byte phase, masks, or arithmetic. Drawing migration must reconcile reuse with its blending implementation later. |
| Archived `simd_rgba_putpixel_batch_supported` | `put/helpers/put_pixel.rs::rgba_run_facts` plus `StrategyGuard::RgbaPixelWriteRun`; the same mode declaration drives assessment and preparation. |
| Archived `simd_rgba_putpixel_batch` and `kernels/rgba_batch.rs::apply_rgba_batch` | Typed RGBA architecture owners compose `put/helpers/put_pixel_vector.rs::{batch,apply_rgba_batch}`; immutable admitted writes retain order, one frame clone, and vector-store telemetry. |
| Archived PutPixel inline vector tests | Two source regressions relocated to `put/tests/put_pixel.rs`; pool-level telemetry/resource behavior still needs review against the maintained executor. |

Physical-lane source update (2026-11-03): `Mode` now owns Pillow pixel size,
byte-carrier pixel width, and omitted-lane offsets; Paste and Composite derive
their byte layout from those declarations. Compact LA/La/PA and
RGB/YCbCr/LAB/HSV PutPixel owners preserve lane values in Default and ordered
batch paths. The six identities with a declared SIMD single-write strategy also
restore the omitted-byte payload after the existing vector store. This is
source-only; parity cases and selected-architecture receipts remain pending.
Bulk PutData and LA/PA PutAlpha/PutAlphaData now restore their affected lanes;
Draw and other mutable or mode-preserving producers still need lane propagation
review before downstream Paste/Histogram parity is complete.

Remaining put-family work includes typed wrapper boundaries and complete caller reconciliation. PutData and both alpha operations have source owners, but remain unvalidated and are not end-to-end complete. The structure guide remains the placement authority. No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage has run. The pipeline output-mode correction remains runtime-unverified. The active CPU refactor remains incomplete.

### PutData source progress: incomplete and unvalidated

- Source review confirmed that PutData's payload layout can differ from the source image in direct core calls. The archived CPU operation converts to the requested byte layout in that case; GPU admission explicitly retains this CPU fallback. The refactor therefore retains a typed payload parameter independently of the source variant rather than removing it as a redundant mode hint. A complete review of all 25 Default mode owners against `legacy/ops/put.rs::op_put_data` confirmed the remaining cross-carrier `ConvertBuffer` calls implement that behavior: they create the requested 1/2/3/4-channel carrier before applying the original byte prefix. Same-carrier writes use the exact native carrier and preserve its mode identity; no conversion remains solely for kernel reuse. This source review is unverified.
- `put/ops.rs::PUT_DATA_SUPPORT` now classifies every source identity through the same declaration that binds its concrete prepared plan. All 25 sources have serial implementations. Their payload contract enumerates all 16 byte-backed mode identities, including La and LAB's native two- and three-channel layouts and RGBA's RGBa/RGBX aliases. The nine internal high-depth payload identities have an explicit rejection; this does not broaden the public wrapper's accepted strings or replace its I;16 mutation path.
- Each mode's Default implementation receives its exact buffer type and a prepared payload strategy. It retains the original nonempty full-payload replacement paths for matching native layouts; 16 formerly clone-only index/color-space/raw-four-byte writes now use the native prefix helper. Cross-layout typed conversions, I;16 clipping, prefix truncation, untouched suffix, zero-size behavior, and `putdata: buffer error` failure remain. No source-mode or channel inference occurs inside the conversion or copy loops.
- Output relations are explicit in the mode contract: a matching native byte layout preserves the exact source identity; a cross-layout or high-depth conversion returns the corresponding L/LA/RGB/RGBA carrier. The same payload declaration generates accepted identities, compatible native widths, payload strategy construction, and the inspectable output table.
- All 16 byte-backed source identities have native-copy SSE2, NEON, and wasm SIMD128 owners. This retains the archived adapter's logical-mode fallback, including its La/LAB physical-copy admission through an explicitly supplied compatible payload. The nine high-depth sources use their serial strategy. Exact source length and compatible payload identity guard specialization; a missing/ineligible architecture selects that same source mode's Default strategy. The original u32 byte-count error, full-payload single copy, partial source/prefix copies, padded vector tail, counters, and empty finalization remain in `put/helpers/put_data_vector.rs`.
- Registry PutData registration now returns its family-owned prepared plan. Its legacy `op_put_data` facade import was removed. `PipelineOp::PutData` and its request facts carry exact `Mode`; CPU contracts and GPU consumers select behavior from that identity. GPU maps supported modes to existing shader layout codes. `PutAlpha` and `PutAlphaData` now carry exact `Mode` as well, with GPU retaining only the numeric shader layout mapping.
- Typed native byte finalization is now shared through `image_utils::typed_bytes_to_buffer`. The existing raw-byte facade delegates to this same length/truncation/shape implementation, and PutPixel no longer duplicates it. Concrete operation strategies construct the exact result variants. Original raw-byte error strings and checked nonempty/empty policies are retained.
- Bulk PutData now carries optional omitted-lane bytes on `PipelineOp::PutData`. Pillow-normalized `putdata_values` supplies the packed or component-derived physical lanes; the selected CPU mode/architecture strategy restores those lanes after its logical byte-prefix kernel, preserving the untouched suffix. Raw-byte PutData supplies no lane payload and reconstructs mode defaults for touched pixels. The post-kernel reconciler also retains LA/La/PA and RGB/YCbCr/LAB/HSV lanes when direct compatible-payload routes preserve those source identities. The GPU contract declines compact-carrier PutData until a GPU result strategy preserves those lanes. This source-only path remains unvalidated; no parity or benchmark evidence is claimed.

| Preserved source/function | Maintained destination and disposition |
| --- | --- |
| `legacy/ops/put.rs::op_put_data` complete-payload native branches | Exact source mode `default/put_data_impl.rs` owners compose `put/helpers/put_data.rs::native`; nonempty replacement and fallback construction order retained. |
| `Image::putdata_value_at` immediate host-callback mutation | `image/data_access/put.rs` retains normalization and callback ordering; after loading, the exact planned `Mode` is checked against `DynamicImage::mode()` and the raw byte carrier is mutated through the exact-mode `PutDataValueAt` contract. LA/La/PA and RGB/YCbCr/LAB/HSV restore their producer-specific omitted lanes from the original `getink` value, while preserving untouched payload entries. The operation currently has only a native Default owner and records that fallback without delaying the write. Source review confirms retained `LoadedData` and P palette metadata; broader caller review remains pending. |
| `Image::putdata_l16_bytes` raw-byte fast path | `PutDataL16Bytes` selects the exact I;16 mode and updates only the supplied raw row-prefix bytes in place, preserving existing values outside that prefix. It consumes Python's reported entry count, clips scaled input bytes, and applies exact native/little/big-endian lane order without staging a replacement frame. Bytes-subclass overreads and scaled values whose C double-to-int conversion is undefined are marked `NO_PILLOW_SUPPORT`; deterministic Rust extension behavior is tracked separately from Pillow parity. Source: [Pillow 12.2.0 `_putdata`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L1531-L1551). Source review only; partial/full-row parity and receipts remain pending. |
| Numeric I;16 `Image::putdata_value_at` path | `put/helpers/put_data_value_at.rs::luma16_sample` now mirrors Pillow's `(int)value` followed by independently clipped low/high bytes, then writes the resulting logical sample to the native u16 carrier. This fixes the direct float-to-u16 cast for negative values, values above 65535, and failed numeric coercions. NaN, infinity, and values outside the C-int range are undefined in Pillow; the deterministic Rust cast is marked `NO_PILLOW_SUPPORT` and belongs to the target-only extension lane. Source: [Pillow 12.2.0 `_putdata`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L1572-L1584). Pillow parity and receipts remain pending; no checks ran. |
| `Image::putpixel_mode_scalar` numeric I/F immediate write | `put/ops.rs::PUT_PIXEL_VALUE_AT_SUPPORT` classifies all 25 identities and links I/F to CPU Default mode owners. The wrapper normalizes the same four native bytes, selects from `DynamicImage::mode()`, mutates in place through the shared raw-byte helper, and records its selection. Source-only; unverified. |
| `op_put_data` partial and cross-layout branches | Source review against the archived `img.to_luma8`/`to_luma_alpha8`/`to_rgb8`/`to_rgba8` branches confirms that cross-carrier `ConvertBuffer` initialization is operation semantics before prefix replacement. Same-carrier writes use `put/helpers/put_data.rs::native` and retain exact source mode identity. No DynamicImage reclassification occurs in the loop, and no conversion is used only for kernel reuse. |
| Existing I;16-to-byte raster conversions reached by `op_put_data` | Four endian-identity owners compose the original clipped sample traversal through `put/helpers/put_data.rs::clipped`. |
| Archived vector `native_put_data_layout` and shape/image admission | Mode-owned `PUT_DATA_SUPPORT`, `ByteDataPayload`, and `PutDataNative` declarations bind source identity, explicit compatible payloads, exact native storage, and prepared architecture selection. |
| Archived `simd_put_data` and `native_bytes.rs::copy_native_bytes` | `put/helpers/put_data_vector.rs`, composed by exact source/architecture owners; original allocation and native byte traversal retained. Reconcile broader native-copy reuse as other families migrate. |
| `image_utils.rs::raw_bytes_to_image_with_empty` shape finalization and PutPixel's interim duplicate | One typed buffer finalizer in the existing canonical utility, reused by both raw facade and typed PutPixel/PutData owners. |

### PutAlpha and PutAlphaData source progress: incomplete and unvalidated

- The full source conversion review compared all 25 source identities' declared outputs and current Default owners against `legacy/ops/put.rs::{op_put_alpha,op_put_alpha_data}` and `legacy/ops/color.rs::op_extract_band`. The archived CMYK promotion, source-carrier changes, palette index/alpha handling, high-depth narrowing, and mask-to-L behavior are accounted for. Existing conversions remain only where the operation requires them; required output allocations/copies remain. This source review does not establish compiled behavior or parity.
- `pipeline_output_mode` now uses the same `known_putalpha_mode` mapping as lazy mode planning. A queued L→LA, RGB→RGBA, P→PA, or CMYK→RGBA mutation therefore declares its actual output identity instead of caching the pre-operation source mode. Source-only; unvalidated.
- Each operation now has an exhaustive 25-mode entry-point declaration, explicit outputs, typed source buffers, and a concrete Default owner. Constant alpha keeps native L/LA/P/PA layouts, promotes RGB/CMYK/other byte carriers as before, clips I;16 samples through the existing helper, and retains the CMYK MULDIV255 conversion path.
- The original architecture loops are placed in mode-owned folders: constant alpha has vector owners for L, P, RGB, RGBA, RGBX, and CMYK; image-backed alpha has L/LA/P/PA/RGB/RGBX/CMYK owners. The same declaration binds build and architecture requirements, native-byte guards, minimum vector groups, and same-mode Default fallback. RGBA mask replacement remains in Default because its archived “SIMD” path only cloned and wrote alpha bytes; it does not claim a vector implementation.
- PutAlphaData retains the accepted One/L mask identity in its borrowed strategy and support contract. Both native carriers supply the same one-byte luma samples used by the legacy normalization, while unsupported mask identities keep the existing `illegal image mode` rejection. Architecture guards accept either exact native mask mode and preserve the large RGB-source row path, complete vector groups, scalar tails, and original receipt counters.
- CMYK alpha calls the shared serial RGB conversion helper, so CPU execution does not enter the optional Rayon route. The shared u16-to-byte vector packing primitive is now owned by the CPU channel helper and reused by grayscale and alpha kernels.
- Registry callbacks now prepare opaque operation plans and no longer import the old alpha operation facades. Existing alpha kernel assertions were relocated with their original inputs and expectations; they were not run.
- The maintained serial CPU pool and prepared batch dispatcher receive routed context and execute registered operation plans in order. The Pillow wrapper passes resolved palette metadata through prepared CPU batches, including automatic SIMD-to-CPU recovery; each operation receives it only while its current raster identity is P or PA. Consecutive PutPixel runs use the prepared PutPixelBatch mode/architecture declaration, retain serial Default, account one output boundary, and report the fusion count. Consecutive transpose runs compose into one prepared mode/architecture strategy; cancellable runs preserve the packed-storage guard and defer the owned identity copy to the batch terminal, which records the allocation and boundary. Consecutive L/LA/RGB/RGBA point operations compose native byte LUTs and enter the prepared Eval strategy; identity LUT chains retain a single owned copy and report `native-identity` without claiming a kernel selection. Multiply+Screen uses its prepared native Chops fusion; AlphaComposite+Mirror, Draw runs, and RGB PutPixel+Thumbnail use their prepared CPU strategies and declared guards. Owned Invert and Chops Add/Sub use the selected in-place route only when its architecture guard admits it, otherwise the prepared allocating strategy. Owned Invert now restores zero omitted lanes for ImageOps point semantics and inverts retained lanes for Chops semantics after the selected in-place kernel. Batch source review is complete; parity for outputs, raw-byte follow-ons, resource boundaries, selection/fallback receipts, tails, and errors remains pending. Canonical pipeline/binding mode boundaries, metadata transitions, registry-wide assessment, and remaining operation families also remain open. No compiler, formatter, checkstyle, lint, test, parity, benchmark, or coverage has run; this source work is incomplete and unvalidated.

### Transpose-family source progress: source review complete; runtime unvalidated

- Archived-route reconciliation: byte Flip's prior SIMD adapter called native
  `flipv()` and had no distinct vector kernel; the maintained byte Default
  owners retain native row copying, while I;16 keeps its vector Flip/Mirror
  strategies. Transpose's archived adapter consumes the logical source prefix
  for nonempty native-byte images and accepts empty native-byte images only
  with empty storage. I;16 transpose uses the logical prefix for nonempty
  inputs and retains Default fallback for empty inputs. The maintained mode
  guards, wide kernel, and RGBA AVX2 kernel preserve these boundaries.
- Caller review: Rust `Image::transpose_with_input` validates integer method
  IDs and maps all seven values to `TransposeMethod`; `Image::transpose` parses
  the corresponding names before queuing `PipelineOp::Transpose`. ImageOps
  `flip` and `mirror` queue their own pipeline operations. Single operations use
  the registered prepared strategy; adjacent transpose runs compose through
  `TransposeTransform` and prepare the composed method from the same injected
  `ExecutionContext`. A canceled run records `native-identity` without an
  architecture only when the backing storage exactly matches the logical
  raster; otherwise it falls back to individual operations so their normal
  output normalization occurs. GaussianBlur's RGBA transpose helper also
  prepares the same mode/architecture strategy with the filter's injected
  context. P/PA raw samples, palette bytes, and palette alpha are retained by
  the lazy image wrapper.

- `transpose/ops.rs` declares exhaustive operation-level support for Flip, Mirror, and Transpose with typed mode dispatch, prepared plans, registry callbacks, and receipts. Every exact raster identity has a Default owner and preserves its mode tag. Pipeline registration no longer passes a Pillow mode string into these operations.
- The byte-backed mode owners use checked native row/pixel copying and fall back to the same generic typed image transforms for malformed or nonstandard storage. High-depth owners preserve typed sample values. Palette indices, premultiplied samples, color-space bytes, numeric carriers, and byte-order mode identities are moved without conversion.
- Transpose covers all seven methods with exact-mode Default owners and typed method preparation. Its native byte path retains checked dimensions, same-dimension row copying, serial/tiled axis swaps, the original 32-pixel tile threshold, and the RGBA 16-pixel tile size. Typed modes use the original mode-preserving pixel-coordinate transforms.
- An AVX2 RGBA mode strategy consumes the injected architecture plus exact mode, logical-byte-prefix, dimension, and axis-swap guards. Its `rgba8/avx2` kernel ports the archived AVX2 tile transform and handles incomplete edges through byte copies. This path has not been compiled or parity-checked.
- The archived wide-lane Mirror kernel is shared in `transpose/helpers/native_mirror.rs`; `One`, `ImageLuma8`, and `ImagePalette8`; `ImageLumaA8` and `ImagePaletteAlpha8`; `ImageRgb8`, `ImageHsv8`, and `ImageYCbCr8`; and `ImageRgba8`, `ImageCmyk8`, `ImageRgbPremultiplied8`, `ImageRgbx8`, `ImageI32`, and `ImageF32` bind it through exact-mode declarations for SSE2, NEON, and Wasm SIMD128. Each prepared strategy restores its own `DynamicImage` variant. RGBA uses the same helper in its custom AVX2-transpose strategy owner. Native row length and channel-group guards select these implementations, with the same-mode Default mirror as fallback. The `ImageI16`, `ImageI16L`, `ImageI16B`, and `ImageI16N` Flip/Mirror native-copy routes now use `transpose/helpers/native_luma16.rs`, preserving typed samples and exact output identities under SSE2, NEON, and Wasm SIMD128 strategies with same-mode Default fallback. The packed-word RGBA Rotate270 kernel runs in its Default owner behind the original x86_64, tile-threshold, and four-pixel divisibility guards; it uses ordinary native-endian words and no optional instruction set. The archived portable `wide` transpose tile, gathered-row, and scalar-edge paths are migrated to `transpose/helpers/native_transpose.rs` and selected by exact-mode SSE2, NEON, and Wasm SIMD128 strategies, with same-mode Default fallback. Its conditional Rayon row collection is excluded from CPU and remains for the later `pool_parallel` stage. The archived byte Flip adapter had no separate vector kernel, so no distinct architecture route is required. All direct, fused, nested-filter, registry, and palette-metadata caller edges listed above have been source-traced. No compiler, formatter, lint, test, parity, benchmark, or coverage command has run.

| Preserved source/function | Maintained destination and disposition |
| --- | --- |
| `legacy/ops/imageops/pixel.rs::{flip_native_rows,op_flip}` | Exact-mode Default implementations under `transpose/<mode>/default/flip_impl.rs`, sharing `transpose/helpers/pixel.rs::flip_vertical_bytes`; generic typed fallback preserves the old fallback behavior. |
| `legacy/ops/imageops/pixel.rs::{mirror_byte_row,mirror_byte_rows,op_mirror}` | Exact-mode Default implementations under `transpose/<mode>/default/mirror_impl.rs`, sharing checked native byte reversal; typed fallback matches the original generic horizontal transform. |
| `legacy/ops/transpose.rs::{execute_transpose,transpose_bytes_serial,transpose_bytes_tiled,transpose_bytes_rows}` | `transpose/helpers/pixel.rs` plus each mode's `default/transpose_impl.rs`; serial byte layout and tiling behavior are retained without Rayon. |
| `PipelineOp::{Transpose,Flip,Mirror}` registry callbacks | `transpose/ops.rs` prepares the exact mode strategy with injected `ExecutionContext`; registration no longer supplies a string mode. |
| `legacy/ops/transpose/kernels/transpose_rgba/x86_64.rs` | Migrated to `transpose/rgba8/default/rotate270_tiled.rs`; it runs only under the original x86_64, tile-size, and divisibility guards and remains part of the Default strategy because it has no ISA requirement. |
| `legacy/pool_simd/ops/adapters/transpose/kernels/rgba_avx2.rs` | Ported to `transpose/rgba8/avx2/transpose_impl.rs`; declared through the `TransposeRgbaAvx2` guard. Source is unvalidated. |
| `legacy/pool_simd/ops/adapters/flip.rs::simd_flip` byte route | It delegates to `img.flipv()` and has no distinct vector kernel; the exact-mode Default owner retains native row copying. Do not add a separate architecture strategy for this adapter. Its I;16 native row-copy route is in `transpose/helpers/native_luma16.rs`. |
| `legacy/pool_simd/ops/adapters/flip.rs::{simd_flip,simd_mirror}` I;16 native-copy paths | `transpose/helpers/native_luma16.rs`, selected by the exact `ImageI16`, `ImageI16L`, `ImageI16B`, and `ImageI16N` Flip/Mirror strategy declarations. Native byte length must be exact and nonempty; each mode's same-mode Default implementation remains the fallback. |
| `legacy/pool_simd/ops/adapters/flip.rs::simd_mirror` byte-backed modes | `transpose/helpers/native_mirror.rs`, selected by exact per-mode declarations for `One`, `ImageLuma8`, `ImagePalette8`, `ImageLumaA8`, `ImagePaletteAlpha8`, `ImageRgb8`, `ImageHsv8`, `ImageYCbCr8`, `ImageRgba8`, `ImageCmyk8`, `ImageRgbPremultiplied8`, `ImageRgbx8`, `ImageI32`, and `ImageF32`; RGBA uses its custom strategy owner. |
| `legacy/pool_simd/ops/adapters/transpose/kernels/portable.rs::{transpose_gathered_row,transpose_native_tile,transpose_native_row_group,transpose_native_tiled}` | Portable wide-lane tile, gathered-row, and scalar-edge paths are in `transpose/helpers/native_transpose.rs`, reached through exact-mode SSE2, NEON, and Wasm SIMD128 owners with same-mode Default fallback. Native-byte modes preserve logical-prefix handling for nonempty backing storage and exact empty storage; I;16 preserves prefix handling only for nonempty inputs. The legacy conditional Rayon row scheduling and collection helpers remain archived for the later `pool_parallel` stage. |
| `legacy/pool_simd/ops/adapters/transpose.rs::rgba_avx2` and `kernels/rgba_avx2.rs` | Ported to `transpose/rgba8/avx2/transpose_impl.rs`; declared through the `TransposeRgbaAvx2` guard. Source is unvalidated. |

### Transform-family source progress: source review complete; runtime unvalidated

- `transform/ops.rs` declares all 25 exact input identities at the family entry point. Twenty Pillow modes have native Default strategies; the five internal high-depth identities retain explicit `NotImplementedError` rejection. Every maintained byte result preserves its exact input variant. Default affine-nearest owners retain the archived specialized L, LA, and three-byte RGB-layout kernels; RGB, HSV, and YCbCr share only the same-layout `Rgb` sampler while preserving their exact wrappers. The I32/F32 owners retain their raw four-byte carriers and their typed scalar filtered paths. Four I;16 identities have distinct mode owners using typed u16 buffers and explicit native/little/big byte order; filtered I;16 behavior preserves Pillow's byte-plane filter semantics without creating or routing through another image mode.
- Public Transform entry points in `ops/transform.rs` create `PipelineOp::Transform`, and `registry/registration/transform.rs` prepares the mode-aware CPU owner. `transform_public` obtains the cached exact `Mode` through `mode_identity()` and passes it into fill normalization and palette/One filter selection; fill arity, scalar, carrier, and named-color resolution match enum variants. Pillow's public `fill` parameter is documented as unused for built-in method IDs; `fillcolor` initializes outside pixels and its omitted/present state flows to the selected strategy. Python and JavaScript adapters use the exact `mode_identity()` for P-specific fill parsing instead of branching on `Image.mode()` text. The typed `color::getcolor_for_mode` preserves the existing Pillow string resolver's result for all 20 externally named identities without spelling round-trips and preserves carrier-equivalent named-color results for the five internal identities; operation support still rejects those five modes. Sampling, F sample-word and omitted-fill, P forced-nearest, PA bicubic-filter, LA/RGBA/RGBX identity, I;16 byte-order, and typed color resolver behavior belong in the mode-by-architecture parity matrix. `ops/transform/fill.rs` handles P fill identity/color normalization and its type/value errors. Architecture evidence must cover declared SSE2/NEON selection where compiled, unavailable-architecture fallback, and Default when a mode has no specialization. The remaining legacy suite stays as reference; no supported active caller needs its `explicit_mode` override.
- The Python facade exposes Pillow-compatible `Transform`, `ImageTransformHandler`, and `SupportsGetData` names. Handler objects are dispatched in the host layer; the LA/RGBA premultiplied-image step applies to this custom-handler route, while built-in numeric methods stay on the core's fused native alpha strategy. Objects exposing `getdata()` are resolved before the numeric method/data pair reaches PyO3. Keep handler error ordering, `getdata()` result forwarding, LA/RGBA custom-handler mode, and direct numeric-mode behavior in frozen CPU parity; implementation is source-reviewed but runtime-unvalidated.
- Empty-mesh P scalar-fill returns the Pillow-required source palette and image metadata with its new index raster; this immediate path does not materialize or enter a CPU kernel. Add a frozen parity case with a non-default source palette to verify this output metadata.
- Mode owners inject `ExecutionContext` into their prepared strategy. Architecture selection occurs from each mode's declaration. All 14 native-byte modes admitted by the archive (`1`, L, P, LA, PA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, HSV, I, and F) declare AVX, AVX2, SSE2, NEON, and Wasm SIMD128 routes for affine and perspective/quad nearest through `TransformByteKernels`. L, LA, RGB, HSV, YCbCr, RGBA, and CMYK also declare affine bilinear through the same exact-mode support contract. Their wrappers use the shared interpolation helper; only LA and RGBA select alpha-aware premultiply/unpremultiply behavior. Each mode uses its exact typed wrapper and retains its output identity. LAB and La retain their same-mode Default strategy because neither is admitted by the archived native-byte layout contract.
- The four I;16 identities declare AVX, AVX2, SSE2, NEON, and Wasm SIMD128 affine nearest/bilinear routes with a typed u16 output wrapper and an exact native two-byte guard. Nearest sampling stays in u16. Pillow's archived filtered I;16 contract processes the first `width` bytes of each native row and restores the second fill byte; the typed mode owners preserve that byte-plane arithmetic in a scratch buffer while input and output remain exact I;16 variants, with no `DynamicImage` mode conversion.
- The archived adapter's admitted byte routes are affine nearest for the 14 native byte identities `1`, L, P, LA, PA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, HSV, I, and F; affine bilinear for L, LA, RGB, HSV, YCbCr, RGBA, and CMYK; projective/quad nearest and mesh nearest for those 14 byte identities; and affine nearest/bilinear for all four I;16 identities. Projective/quad bilinear kernels exist in the archive but are not admitted by its support predicate and remain out of scope. No active mode strategy may restore the old image-mode inference or carrier retagging.
- The archived mesh entry passes admission for nearest native-byte images, but its loop evaluates each sample scalarly and only groups eight pixels for bookkeeping; it has no vector kernel. Keep mesh in each mode's Default strategy rather than advertising it as an architecture specialization.
- Source review compared Python TransformData classification and public method/error routing with the pre-migration wrapper. Its malformed-input and method-specific error branches remain unchanged; mode selection and fill normalization now use exact `Mode`. Palette fill, P forced-nearest, and PA filtered behavior remain parity cases. Mesh remains Default because the archived implementation only groups scalar samples and has no vector kernel. No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage command has run for this family.
- Transform architecture source audit (2026-11-02): all 20 accepted mode owners have a Default implementation. Every declared non-Default Architecture has a corresponding exact-mode execute arm and implementation file; the five internal CoreOnly modes remain explicitly rejected at the operation contract. The shared selector preserves same-mode Default fallback with its selection reason. This is source inventory only; architecture execution and fallback receipts remain unverified until the frozen CPU parity matrix.

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
  borrowed-mask/moved-input boundary. The palette-present P Text strategy
  retains the archived source-mask dimension cap; negative-origin behavior is
  recorded as a parity TODO rather than changed during this structural pass.
  Draw's support facts include palette knowledge for both P and PA. Direct PA
  Text/Bitmap keep the native index-alpha raster and restore the wrapper's
  optional palette, palette-alpha, format, info, and EXIF metadata after the
  operation. PA Text now has its own native index/alpha loop rather than using
  LA's luma conversion: Pillow's packed integer ink reaches PA's P and A bytes,
  and its L-mask route accounts for destination alpha. `resolve_integer_color`
  now preserves the packed low four bytes for PA integers. The PA Bitmap writer
  applies L-mask coverage to the index and hidden index lanes, promotes any
  nonzero L coverage to full coverage when destination alpha is zero, and blends
  alpha with the original coverage. RGBa Bitmap masks use their stored alpha as
  coverage and route each stored destination byte through PREBLEND; zero-alpha
  RGBa samples still execute because PREBLEND includes the source contribution.
  Both behaviors are implemented in `BitmapMask` and exact-mode owners, matching
  Pillow 12.2's `Paste.c` fill paths by source inspection. This closes those two
  source-review items only; their frozen parity cases remain pending.
  SIMD source comparison is complete: Draw's DIV255 and pack kernels retain
  their archived arithmetic, and the span, line, shape, and rounded-rectangle
  kernels retain their archived geometry and tail behavior. The rectangle
  fill/outline loops were extracted unchanged; architecture wrappers now carry
  an exact `DrawLayout` and reconstruct the output with its exact `Mode` rather
  than infer a variant from channel count. Direct Bitmap/Text mode contracts,
  wrapper error ordering, and P/PA palette transitions remain open.
  Remaining source review compares every direct Bitmap/Text mode's ink and
  mask rules, P/PA metadata, I/F raw carrier behavior, and error ordering
  against the archived/wrapper contracts, then verifies each vector kernel
  and retained architecture admission route against the preserved SIMD sources.
  No unit tests are planned for this migration; correctness evidence is parity
  across operation × exact Mode × available Architecture, including selection,
  fallback, and rejection receipts. No build, formatter, checkstyle, lint,
  parity, benchmark, or coverage command has run for this family; source review
  remains in progress.

### ImageOps source progress: incomplete and unvalidated

- Public ImageOps mode admission in `ops/imageops/point.rs` now matches typed `Mode` identities rather than Pillow strings. Exact mode lookup uses the wrapper's cached identity; when a lazy pipeline has no identity yet, the current materialization fallback is explicitly pending removal once all pipeline transitions preserve a known exact mode.
- `imageops/ops.rs` owns an exhaustive classifier for all 25 exact modes. The byte modes have per-mode strategy owners and preserve their exact mode identity. Numeric I/F and internal high-depth carriers retain their existing shape-error behavior for bytewise Invert, Posterize, and Solarize. Invert's P, LA, PA, RGBA, and CMYK routes are explicitly CoreOnly because the Pillow wrapper rejects those identities; each owner has a nearby `NO_PILLOW_SUPPORT` comment. The P/PA routes invert native samples and keep attached palette metadata, so indexed materialization must not expand them before prepared dispatch.
- CropBorder declares CoreOnly support for the five internal high-depth identities with nearby `NO_PILLOW_SUPPORT` comments and preserves each exact raster mode. Colorize declares Pillow support only for exact `ImageLuma8`; its other 24 mode identities are explicitly rejected with the same empty `AssertionError` as the public wrapper. This removes the legacy non-L-to-L projection from the prepared path and keeps Colorize native to its actual Pillow input mode.
- Each mode's `default/invert_impl.rs` performs the same `255 - byte` transform as `legacy/ops/imageops/pixel.rs::op_invert`; the scalar byte loop is shared in `imageops/helpers/pixel.rs`. Strategy preparation receives `ExecutionContext`, and registry preparation now returns the closed `PreparedOperation` plan.
- Named Expand and Pad colors now stay on the typed `Mode` path through `color::getcolor_for_mode`; they no longer format a Pillow spelling only to resolve the same mode again. The shared resolver marks its internal high-depth color results `NO_PILLOW_SUPPORT`; operation support still decides whether a core-only route can execute.
- Source review compared the archived native-byte adapter's exact One/L/P, LA/PA, RGB, and RGBA/CMYK admissions and its SSE2/NEON/Wasm SIMD128 wide-kernel path. Maintained mode owners retain native-byte inversion and same-mode output. The Pillow wrapper rejects P/LA/PA/RGBA/CMYK, so their explicit `NO_PILLOW_SUPPORT` routes are CoreOnly. P/PA now stay indexed, invert their native samples, and retain attached palette metadata. Maintained LA/PA/RGBA strategies invert every stored byte; the archived ImageOps SIMD adapter preserved alpha in those modes. CMYK inverts all four bytes in both paths. The separate SIMD registration is no longer active.
- The public Invert wrapper and point-run fusion callers have been source-reviewed; fusion composes LUTs through prepared Eval under the same exact mode and injected context. The archived owned-buffer in-place Invert path is restored for Invert and InvertChops: exact-mode support and injected context select the architecture before the owned intermediate is mutated in place; caller-owned input remains untouched. Operation × exact Mode × Architecture parity remains pending, including CoreOnly routes through target-side parity cases.
- Posterize and Solarize now declare all 25 modes at `imageops/ops.rs`, with prepared strategies for L, RGB, and core-only P. P applies each transform directly to index bytes, preserves its palette, and is marked `NO_PILLOW_SUPPORT` because Pillow's ImageOps `_lut` rejects that mode; other unsupported identities retain Pillow's mode-specific `OsError`. Default kernels are mode-owned; L, RGB, and P declare SSE2, NEON, and Wasm SIMD128 strategies using the exact-mode `NativePointwiseBytes` guard and shared vector transform. Prepared execution matches the selected architecture. Public wrappers queue typed operations; custom/high-threshold Solarize uses single-band Eval, whose palette-aware pipeline keeps P indexes and metadata attached. Point-run fusion composes through prepared Eval under the exact mode and injected context. Operation × exact Mode × Architecture parity remains pending, with P handled as a target-only core extension.
- High-threshold and custom-comparison Solarize requests deliberately use `eval_replicated` and enqueue Eval rather than `PipelineOp::Solarize`; the CPU registry prepares them with the mode-aware Eval owner. The remaining SIMD compatibility route is tracked with Eval below.
- `Autocontrast` and `Equalize`/`EqualizeMasked` now have operation-owned support declarations and prepared CPU callbacks. L and RGB use their exact Default kernels and guarded SSE2, NEON, or Wasm SIMD128 kernels. P Autocontrast is CoreOnly in Rust and transforms native indices while preserving P metadata; the Python facade retains its existing `NotImplementedError`; the P Equalize strategy consumes the injected palette, expands to RGB during preparation, and selects its declared RGB data kernel under the P strategy. CPU materialization now reaches this typed P route before generic palette expansion; its declared result is RGB, with palette alpha discarded by the RGB conversion. PA Equalize remains rejected. Source comparison against the archived CPU and SIMD routes covers histogram admission, cutoff/equalization arithmetic, LUT application, masks, empty images, and direct callers; masks accept exact `One`/L identities and preserve mode-before-size error ordering. CropBorder also routes through its prepared imageops callback; its archived ordinary-byte admission and empty/error ordering are represented. Operation × exact Mode × Architecture parity remains pending.
- `Expand` now has an operation-owned support declaration for all 25 exact source identities. Each mode has a Default owner under its own folder, performs border fill and row copying in its native sample representation, and returns the same exact `DynamicImage` variant. P/PA preserve their index samples; typed I;16 and internal high-depth modes never route through an 8-bit mode. The five internal high-depth modes are core-only and carry nearby `NO_PILLOW_SUPPORT` comments; fill bytes become same-valued native numeric samples. `ExpandedDimensions` exposes source layout, border facts, and checked output allocation through the same contract used during preparation. Registry preparation now enters the closed CPU `PreparedExpand` plan instead of the archived direct callback.
- The new `geometry/ops.rs` owner declares typed support for Contain, Cover, Fit, and Scale across all 25 exact identities. It preserves the archived aspect-ratio rounding, zero-size error order, Fit bleed/centering box, and P nearest-filter rule, then composes the request with a prepared exact-mode Resize plan using the same injected context. Its support descriptors name Resize or ResizeBoxed as child contracts, and the prepared operation exposes each child's selected architecture alongside the outer geometry selection. Pad now routes through its prepared exact-mode owner described below. The five internal modes carry nearby `NO_PILLOW_SUPPORT` declarations. Geometry parity and archived SIMD route comparisons are not complete.
- Pad now has an operation-owned prepared entry at `pool_cpu/ops/geometry/pad/ops.rs`, with exhaustive support for all 25 `Mode` identities and a typed `PadParameters` request contract. Each mode has its own `ops.rs` plus serial `default/pad_impl.rs`; implementations preserve the exact output variant and use that carrier's native samples. P forces Nearest in its composed Resize child. The same support declaration composes Resize, and the prepared Pad receipt exposes the child's concrete selection. The five internal high-depth identities include `NO_PILLOW_SUPPORT` comments and use their native carriers. Thirteen exact byte modes declare SSE2, NEON, and Wasm SIMD128 strategies guarded by native byte layout; other modes use same-mode Default. Source review against `legacy/ops/imageops/pad.rs` and `pad/native.rs` accounts for identity-source append eligibility (L/RGB/HSV/RGBA), resized-source append eligibility (L/CMYK/RGBX), native row fill/copy, indexed P/PA samples, nearest P resize, and the legacy fallback paths. Review against `legacy/pool_simd/ops/adapters/pad.rs` and its shared Resize admission accounts for exact byte carriers, empty-source fill, vertical identity fill/copy (including RGBA vector fill), full-width filtered L/RGBX convolution, resized-source composition, and vector fallback. Maintained Pad keeps these paths in typed exact-mode strategies: the vector append route retains the archived mode/source split, and filtered full-width L/RGBX composes through the prepared Resize child only when both selections use the same architecture. Pad now retains known omitted lanes for same-size copied content and mode-derived border fills; when Pad resamples its content, omitted-lane propagation remains a Resize-dependent source TODO. The public Rust, Python, and JavaScript callers converge on the typed Rust ImageOps Pad entry; primitive wrapper validation order was source-reviewed, and no active legacy or `pool_simd` imports remain. Formatting an arbitrary invalid Python filter may run before Pillow ImageOps geometry validation; a source TODO tracks deferring it. The mode/kernel and architecture source comparison is complete; Python F scalar and one-element tuple fills now reach the existing F carrier as native float32 bytes, while tuple/list identity and remaining integer/component/error-order details stay under the PadFrame TODO. CPU parity, compilation, and performance evidence remain pending.
- Pad dispatcher source correction (2026-11-01): exact-mode owners with architecture strategies return vector-work counters, while 12 Default-only owners return only the same-mode image and take no architecture argument. The central dispatcher now preserves each owner signature and supplies zero vector-work counts for those Default-only results. This fixes a source-level type mismatch found during manual review; no compilation or other validation ran.
- Pad fill color source correction (2026-11-02): Pillow `Image.new` accepts two-component colors for `LA`, `La`, and `PA`, while `ImageColor.getcolor` premultiplies the color bands for lowercase-alpha `La` and `RGBa` string colors. The typed resolver now accepts raw `(luma, alpha)` tuples for `La`, rejects RGB-shaped `La` tuples, applies Pillow rounding to named/hex `La` and `RGBa` fills, and preserves the two-band scalar alpha behavior. Updated the existing `La` color-output expectation. Source comparison only; no formatting, build, tests, parity, or benchmark ran.
- Pad fill representation finding (2026-11-02; resolved at source level by the typed Pad fill path below): the original `PadFrame.color` route narrowed every fill to four u8 components, losing upper sample bits for I;16 and high-depth modes. `PipelineOp::PadTyped` now preserves the original scalar/component values through mode-owned preparation. The existing Pad parity TODO still covers mode-specific wrap/default/alpha behavior and fallback. No runtime verification ran.
- The archived 128-bit fill/copy implementation now lives in `imageops/helpers/expand_vector.rs` and is shared by exact-mode byte strategies. The same declaration selects SSE2, NEON, or Wasm SIMD128 when built and available. Its `ExpandVector` guard enables full border fills for one- and two-byte layouts; three- and four-byte layouts use it for border-zero identity copies, matching the archived vector work. Other requests select the same-mode Default implementation. The vector kernel constructs the original exact variant rather than inferring output mode from channel count.
- The archived CPU Expand cases and SIMD byte-fill/copy regressions now live in `imageops/tests/expand.rs`; mode-string overrides were replaced with exact `DynamicImage` variants, and the old contradictory carrier/mode checks are no longer constructible. These tests have not been run.
- Expand source review is complete against the archived CPU and SIMD routes. The public wrapper queues into the exact-mode registry; the architecture guard selects vector fill for one-/two-byte layouts and border-zero copies for three-/four-byte layouts, matching the kernel's actual branches. Exact-mode output identity and all declared modes are retained. Operation × exact Mode × Architecture parity remains pending; no active adapter registration remains.
- The ImageOps implementation and support matrix have not been compiled, parity-checked, or benchmarked. No source-completion claim is made.

| Preserved source/function | Maintained destination and disposition |
| --- | --- |
| `legacy/ops/imageops/pixel.rs::op_invert` | `imageops/<exact-mode>/default/invert_impl.rs`, with bytewise arithmetic in `imageops/helpers/pixel.rs`; successful output preserves the exact `DynamicImage` variant. |
| `legacy/ops/imageops/pixel.rs::{op_posterize,op_solarize}` | `imageops/luma8/default/{posterize_impl,solarize_impl}.rs` and `imageops/rgb8/default/{posterize_impl,solarize_impl}.rs`; exact-mode prepared plans own admission and serial execution. Archived vector kernels are folded into each mode's SSE2/NEON/Wasm SIMD128 wrappers via `imageops/helpers/native_bytes.rs`. Compare the archived SIMD admission behavior with the exact-mode owner; no independent registration remains active. |
| `legacy/ops/imageops/colorize.rs::{colorize_lut,op_colorize}` | Pillow's exact-L LUT construction and execution live in `imageops/helpers/colorize.rs` and the L mode owner; the archived vector kernel is composed by native-L SSE2, NEON, and Wasm SIMD128 strategies. Exact nonempty native-L bytes guard architecture selection, and the 65,536-pixel path split, padded tails, and vector telemetry are retained. The public wrapper validates L before queueing Colorize. Non-L projection through a temporary L raster was removed from the prepared CPU route. |
| `legacy/ops/imageops/{histogram.rs,pixel.rs}::{autocontrast_lut,equalize_lut,op_autocontrast,op_equalize_with_mask}` | `imageops/helpers/histogram.rs` retains histogram arithmetic; `autocontrast.rs` and `equalize.rs` declare mode support, mask admission, and prepared selection. L/RGB Default kernels live under their mode folders; P has a CoreOnly native-index strategy with Default and architecture kernels. Equalize P expands the injected palette to RGB and declares the resulting architecture route. |
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

- Python and JavaScript `Image.point` keep their public mode-string parameters
  at the binding boundary, parse the optional float-output mode to `Mode`, and
  match the source image's exact `Mode` for float-LUT and scalar-callback
  routing. Those decisions no longer materialize source mode strings in the
  wrappers. Eager PointScalar and PointFloat retain their prepared selection in
  telemetry and keep source-graph receipts; exact point/error-order and fallback
  parity remain pending; source-only.
- `eval/ops.rs` now exposes an exhaustive contract for the 25 exact identities. `One`, `L`, `LA`, `La`, `P`, `PA`, `RGB`, `RGBA`, `RGBa`, `RGBX`, `CMYK`, `YCbCr`, `LAB`, and `HSV` have native byte-LUT Default strategies with one table per native band. `I`, `F`, all four Pillow I;16 identities, and the five internal high-depth identities retain explicit rejection because `PipelineOp::Eval` accepts byte LUTs. Direct scalar point transforms now have a separate `PointScalar` support declaration for I/F/I;16 and per-mode Default implementations under `eval/<mode>/default`; their public entry points inject a routed CPU context, and the algorithms retain the original native sample arithmetic.
- Each admitted mode's contract includes an exact `256 * native_channels` LUT length and exact native source-byte dimensions. Its Default implementation preserves the input variant and applies the archived affine-LUT or per-band lookup loop directly to that mode's bytes. P/PA index bytes remain native; the image wrapper continues to own palette metadata. The per-mode Default modules now follow the explicit `mode/default/eval_impl.rs` layout.
- The nine identities admitted by the archived SIMD LUT adapter (`One`, `L`, `P`, `LA`, `RGB`, `HSV`, `YCbCr`, `RGBA`, and `CMYK`) declare SSE2, NEON, and Wasm SIMD128 strategies beside their exact mode contracts. A shared family helper owns the reusable channel LUT kernel; RGB's little-endian AArch64 TBL4 and affine paths now live at `eval/rgb8/neon/eval_impl.rs` and are selected directly by the RGB mode owner. Other modes use the shared portable-wide implementation for their declared architecture, while identity LUTs retain the native-copy path. The kernel helpers contain no Rayon scheduling.
- `PointFloat` now declares exact `One`, `L`, and `P` strategies with a typed 256-entry f64-table contract, exact native-byte shape, and fixed `ImageF32` output. Per-mode Default leaves live under `eval/<mode>/default/eval_float_impl.rs`; the shared native gray-byte to F32 kernel lives under `eval/helpers/point_float.rs`. The public wrapper injects a routed CPU context; each leaf reads its native byte samples directly without converting the source mode.
- Source review compared the legacy operation, native-LUT adapter, batch routing, current public point wrappers, and registry preparation. The archived SIMD admission set is exactly `One`, `L`, `P`, `LA`, `RGB`, `HSV`, `YCbCr`, `RGBA`, and `CMYK`; its channel counts, channel-major LUT layout, exact byte lengths, 16-sample table lookup blocks, padded row tails, and RGB little-endian AArch64 TBL4/affine paths are represented by the corresponding prepared CPU mode/architecture owners. Empty images and unsupported architectures fall back to each exact mode's Default strategy. The old separate registration and mode-string inference are no longer active.
- The source comparison found and corrected a request-facts crossover in the `One`, `L`, and `P` owners: byte Eval now supplies `EvalLut` facts, while PointFloat supplies `PointFloatLut` facts. Direct scalar point callsites, exact-mode validation order, mode-preserving byte outputs, and fixed `ImageF32` float-LUT outputs are accounted for. PointFloat carries the leaf selection through to its eager receipt; scalar point receipts likewise include their selected Default strategy and preserve the input materialization receipts. The archived Rayon row scheduler remains outside maintained CPU execution and is reserved for the later `pool_parallel` pass.
- Expanded parity remains pending across operation × exact Mode × Architecture, including all supported byte modes, vector tail widths, empty inputs, identity LUTs, affine RGB NEON, scalar point modes, and rejected modes. No unit tests or validation commands have run for this source.
- No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage command has run for this source.

### ConvertLab source review complete; parity pending

- `color/ops.rs` declares an exhaustive support contract over all 25 identities. `ImageRgb8` selects the native RGB8 Default strategy and produces exact `ImageLab8`; every other identity retains an explicit `ValueError("image has wrong mode")` outcome. This prepared RGB8-to-LAB strategy is Pillow-scoped. The pinned Pillow 12.2 wrapper uses ImageCms for RGB-family inputs and normalizes other supported sources to RGBA first.
- The public `Image.convert(Mode::ImageLab8)` adapter in `ops/convert/dispatch.rs` normalizes its input before it queues `PipelineOp::ConvertLab` (including the existing I;16 to L to RGB path). Therefore the prepared operation consumes exact RGB8; input mode normalization stays at the explicit conversion boundary. Its RGB byte-length contract permits an empty image only when the native byte buffer is also empty. Commit `5e4b509d4` adds the composed source-to-LAB route to Convert's top-level contract: Pillow identities report Pillow scope, the five internal high-depth identities report CoreOnly, and La retains the exact `conversion from La to L not supported` rejection. The prepared Convert plan captures source→RGB (or I;16→L→RGB) and ConvertLab selections before execution; the wrapper gates its lazy graph with the same source/target declaration.
- The serial Default implementation lives at `color/rgb8/default/convert_lab_impl.rs`. It retains the archived LittleCMS CLUT interpolation, tie-breaking, rounding, and byte packing order, and constructs `DynamicImage::ImageLab8`. Shared CLUT/profile data remains in `lab.rs`; no architecture-specific ConvertLab kernel exists. The old standalone SIMD registration was scalar control only and has been removed from registration.
- ConvertLab source review (2026-11-02): compared the maintained RGB8 byte kernel with commit `3f9dfd403`: its axis table, 33³ CLUT addressing, six-tetrahedra tie-breaking, fixed-point interpolation, `0x8001` rounding bias, and output quantization match. The archived low-level `convert_rgb_image` also accepted non-RGB inputs by calling `to_rgb8`; the active public Convert-to-LAB composition now selects the source-to-RGB strategy explicitly before entering the prepared RGB8-only ConvertLab operation. Direct ConvertLab admission accepts only `ImageRgb8`, declares Default with no architecture-specific kernel, and rejects the other 24 identities. Frozen CPU parity must still confirm these direct and composed routes and their selected-implementation receipts; no checks ran.
- Registry support and prepared-operation wiring now identify ConvertLab through this owner. General Convert remains in progress as detailed below; its public Rust boundary is typed, but the archived SIMD source/target reconciliation and broader caller review are pending. Correctness evidence will come from the operation × exact Mode × Architecture parity matrix; this migration does not add unit tests.
- No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage command has run for this source.

### Convert source progress: source review in progress; incomplete and unvalidated

- Archived alpha route review: see the exact route and empty-input findings in
  “Current source-only continuation.” The broader adapter/kernel map below is
  still pending for ordinary layout and color-space routes, typed byte-source kernels, generic dispatcher guards, build availability, and scalar-control reconciliation
  paths.

- The public Rust `Image::convert` API now takes exact `Mode` for its destination and reads the source via `mode_identity()`. Its dispatch and the conversion helpers use `Mode` matches, including nonstandard color/scalar conversion, binary output, palette output, I;16 routing, and matrix targets. `color::convert_from_nonstandard` now accepts `Mode`. Python and JavaScript parse valid Pillow strings into `Mode` before delegation; an invalid string is retained only as an external compatibility error input so matrix-validation order and diagnostics remain unchanged. Enhance, paste source preparation, and module Composite now pass typed modes to conversion. This removes the previously listed string conversion gap, but does not complete the archived source/target matrix review or prepared strategy caller audit.
- After existing prestandard handling, each Convert source/target pair declared by `ModeSupport` now enters the prepared plan; the same declaration decides eligibility. Source review found `assess_convert_target` rejected typed targets `One` and `P` before checking their declared target lists, which blocked the eager binary and prepared-palette wrappers even after they materialized their source. Removed that stale unconditional rejection so the exact declared target membership now controls admission; this correction is source-only and unverified. Ordinary One conversions use the native One strategy instead of widening through L. Conversion to target One now dispatches through an exact source-mode owner selected from the injected CPU Architecture context; those mode entry points compose the shared Default threshold and Floyd–Steinberg helpers. SIMD requests fall back to Default until a binary architecture kernel is declared. The archived RGB/LAB error-diffusion distinction is preserved. One matrix conversion retains the archived L normalization before entering the prepared matrix strategy. The public matrix path remains eager, but `ConvertMatrix` now has a top-level contract, exact source-mode selection, injected CPU architecture context, and a shared Default implementation that applies the preserved RGB sample projection and matrix arithmetic directly to each mode's native samples without a temporary RGB image. No archived SIMD matrix kernel exists, so SIMD requests use the declared Default fallback; GPU matrix execution remains a TODO beside the wrapper. The matrix strategy admits L/RGB targets for all 25 exact source identities; non-RGB source projections are explicitly CoreOnly. Declared I;16 source pairs for I/F, HSV, and YCbCr now use native u16 strategies; all four I;16-to-P/PA paths now clip native u16 samples directly into identity-palette indices without an intermediate L image. The four I;16 sources now declare cross-identity I;16 targets and execute from their native u16 buffers in their exact mode Default strategies; same-identity conversion preserves samples, while cross-identity conversion retains the archived clipped-L widening result. These target pairs have no architecture kernel and explicitly fall back to Default. LA16, RGB16, RGBA16, RGB32F, and RGBA32F declare the four I;16 targets as core-only routes and project native samples directly into the same luma bytes as their existing Default grayscale strategies, without an intermediate L raster. SIMD requests for these Default-only targets select the mode's Default implementation. LA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, HSV, PA, LAB, I, and F now declare all four I;16 outputs and execute through their exact mode Default strategy; their native pixels are converted directly to the prior L-byte result where needed, without a temporary L image. I32 preserves its target-specific u16 clamp for I;16/I;16L/I;16B and clipped-L behavior for I;16N; F32 retains its clipped byte conversion. One and L keep their existing lazy prepared path. The eager recursive wrapper bridge was removed. `P` retains its exact `ValueError(`conversion not supported`)`, and `La` retains the earlier validation error. Declared RGBa source pairs no longer eagerly unpremultiply and route through RGB; their native source strategy now owns those conversions. RGB→L retains its dedicated Grayscale operation route. CMYK→I/F no longer materializes an intermediate RGB image. Every accepted P/PA source pair now enters its prepared strategy; `La` to other targets remains an explicit validation error. The generic P/PA wrapper quantizers were removed after this source-reachability review.
- `color/convert/ops.rs` now publishes exact typed target contracts for the shared RGBA, RGBa, La, I;16, scalar, and RGBX source groups. The four exact I;16 identities share `I16_INPUTS`; scalar I/F and RGBX have separate contracts. Overlapping target lists do not widen any source's accepted target set.
- Convert identity selection source correction (2026-11-02): `PreparedConvert::execute_with_output` returns the exact source raster for same-mode requests, so an architecture-specific copy kernel was not executed even when a broad target guard could select it. Convert target guards now receive the owning source identity and treat `source == target` as ineligible for architecture kernels. The mode's Default identity path is selected instead, with an `ineligible_request` fallback when a requested architecture would otherwise match. Pixel data and wrapper metadata are unchanged; receipt parity remains pending.
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
  pairs remain under source review. The Image wrapper's dedicated lazy P/PA
  boundary executes the exact prepared strategy and publishes palette/alpha
  metadata after materialization; its parity remains TODO. The generic
  raster-only `prepare_pipeline` path still rejects cross-mode P/PA pending a
  prepared result contract that carries those sidecars. Internal high-depth
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

### RemapPalette source review complete; parity pending

- `color/ops.rs` declares an exhaustive contract over all 25 identities. Only `ImageLuma8` and `ImagePalette8` are supported; both consume their native one-byte samples and return exact `ImagePalette8`. Every other identity is explicitly rejected with the existing `ValueError("illegal image mode")` contract. The public wrapper already admits only L/P and constructs the reordered palette and alpha metadata separately.
- Both Default mode implementations apply the legacy inverse map directly to the source bytes: `inverse[dest_map[new_index]] = new_index`, with all unmapped values remaining zero. No RGB conversion or retagging is used to share code. The shared kernel is in `color/helpers/remap_palette.rs`.
- L and P declare SSE2, NEON, and Wasm SIMD128 strategies with exact one-byte input contracts. The shared portable-vector kernel preserves the archived 16-byte lookup and padded-tail behavior; the serial Default path remains available for every mode. Rayon is not used. The old independent SIMD registration has been removed; prepared CPU registry dispatch selects these architecture strategies.
- `Image::remap_palette_with_source` retains ownership of palette colors and
  alpha. `REMAP_PALETTE_SUPPORT` declares `ImagePalette8` as the output
  for both accepted source modes, and the lazy planner derives the cached
  mode from that contract; the wrapper does not apply a duplicate mode
  override. Map inputs continue to be capped at 256 at the registry
  boundary, matching the previous callback.
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
Mode folders belong only to modes with executable Default strategies; architecture folders belong only to retained kernels. Resize and ResizeBoxed are owned by `resize`; Thumbnail is a separate family and must inject the same execution context into its nested Reduce/Resize work. The five core-only high-depth routes above must not use byte-materializing adapters. Rayon scheduling remains for pool_parallel. The generic Resize compatibility wrappers are test-only; their runtime FMA detector is also test-only. Production F32/I32 AVX/FMA owners obtain X86FmaToken only from the injected execution context, so architecture selection remains context-driven.

`pool_cpu/ops/resize/ops.rs` prepares ordinary and boxed Resize, retains the support selection, and routes exact modes to native owners. CMYK and HSV now have mode-local narrow fixed-point kernels; HSV uses the retained specialization for Bicubic and native three-channel filtering otherwise. One keeps threshold behavior, straight-alpha modes preserve their alpha contract, palette modes preserve indices, and color/padding modes preserve stored channels. Typed I32/F32 and high-depth implementations remain native. The four I;16 modes now have const-specialized endian rules in both Default and wide-vector unboxed paths; empty sources use the zero-output vector path when its guard is satisfied. I32 filtered unboxed NEON has an exact mode/architecture owner and guarded support declaration. All 14 byte modes now have exact SSE2, NEON, and Wasm SIMD128 declarations for guarded ordinary nearest sampling; the shared kernel retains 16-byte shuffle windows, scalar gathers, bounded output stores, and scalar tails. Eleven byte modes plus Luma8 have guarded ordinary filtered vector strategies, and all byte modes have boxed nearest strategies; filtered boxed convolution is declared for all but the One/P nearest-only routes. Remaining Resize work is eager/nested caller context injection and boxed-bound/fault-order review; generic full-operation wrappers in `ops/pil_resize` have no production callers and are test-only. No build or validation has run.

### Thumbnail source review: complete; behavior verification pending

- `thumbnail/ops.rs` exposes the prepared CPU operation and exhaustively lists
  all 25 `Mode` identities. It preserves source identity and passes the same
  `ExecutionContext` into nested Reduce and Resize plans. Preparation selects
  one exact-mode policy: `1`, `P`, and `PA` force nearest; straight-alpha `LA`
  and `RGBA` skip reducing-gap while preserving the requested filter; and
  premultiplied `La` reduces its stored samples like the archived
  implementation. The public `Image.thumbnail` caller computes final
  aspect-preserving dimensions, preserves its materialization/error order, and
  normalizes the indexed-mode filter before queuing. That filter rule is also
  declared by each exact mode strategy, so direct preparation cannot bypass
  it. F32 retains the eligible unboxed Resize route.
  Preparation now stores the
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
  archived branches: `1` and `P` force nearest; straight-alpha `LA` and
  `RGBA` skip reducing-gap; `La` and filtered `PA` requests that reach the
  archive reduce their stored samples. The public PA caller forces nearest
  before queuing, and the maintained PA strategy now owns that normalization.
  `F` and `I` use exact typed scalar-mode Reduce/Resize
  owners; and all remaining identities retain native mode through the same
  composed owners. There are no active callers of the archived Thumbnail
  adapter.
- Archived reducing-gap source correction (2026-11-01; caller reconciliation
  completed 2026-11-02): legacy `prepare_thumbnail_work_image` excludes `La`
  and `PA` from its alpha-mode classification, so a filtered request reaching
  that adapter would reduce their stored samples before Resize. The public
  `Image.thumbnail` caller already forces `PA` to nearest; the exact PA mode
  strategy now owns that same normalization for prepared callers. `La` remains
  the premultiplied-alpha mode that reduces stored samples; only straight `LA`
  and `RGBA` skip reducing-gap. Source-only; CPU parity remains pending and no
  checks ran.
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

### Grayscale strategy review: source complete; runtime unverified

- The operation classifies all 25 exact source identities, rejects the
  unsupported `ImageLaPremultiplied8` conversion with the preserved error, and
  routes registered execution through its mode-owned prepared strategies.
- The wrapper's typed mode planner predicts `ImageLuma8`; eager RGB and lazy
  P/PA materialization call the prepared CPU route. Convert's target-L paths
  use exact source-mode Default leaves where applicable and the declared
  Grayscale architecture kernel for matching vector routes. CMYK, YCbCr, HSV,
  P, PA, I, and F source declarations include L/LA so those public routes reach
  their native source-mode strategies.
- Source search found the generic `execute_default` dispatcher had no callers;
  it was removed so execution stays on the prepared exact-mode strategy path.
- Current Convert source tracing confirms `ops/convert/nonstandard.rs` does not
  perform a generic RGB(A) luma conversion. The remaining
  `color::pil_grayscale_alpha` call in `ops/convert/dispatch.rs` constructs a
  specific `La` result from the declared RGB child in the premultiplied-target
  composition; its source-target route remains tracked under Convert. No
  build, formatter, checkstyle, lint, test, parity, benchmark, or coverage
  command ran.

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
| `image/channel_access.rs`, `compute/pool_cpu/ops/put/ops.rs`, `compute/pool_cpu/ops/analysis/ops.rs` | GetChannel's operation-level declaration covers exact mode band counts, P's palette-preserving identity, I/F rejection before materialization/index validation, CoreOnly classification for five internal high-depth identities, and its composed ExtractBand child. The wrapper reads selector bounds from the declaration, then retains its L identity shortcut, tiny-RGB eager route, backend-pinned lazy route, and P copy. CoreOnly u16 sources now produce `ImageI16` bands; f32 sources produce `ImageF32` bands with little-endian scalar bytes. Carry these routes and the child's selected/fallback receipts into the frozen CPU parity set. |
| `ops/analysis.rs`, `compute/pool_cpu/ops/analysis/ops.rs`, `compute/pool_cpu/ops/analysis/helpers/extrema.rs`, `compute/pool_cpu/ops/analysis/helpers/bbox_execution.rs`, `compute/pool_cpu/ops/analysis/helpers/extrema_execution.rs`, `compute/pool_cpu/ops/analysis/helpers/extrema_bytes.rs`, `compute/pool_cpu/ops/analysis/helpers/extrema_bytes_execution.rs`, exact-mode `default/extrema_bytes_impl.rs` owners, `compute/pool_cpu/ops/analysis/helpers/get_data.rs`, `compute/pool_cpu/ops/analysis/helpers/getcolors.rs`, `compute/pool_cpu/ops/analysis/helpers/getcolors_execution.rs`, exact-mode `default/getcolors_impl.rs` owners, `compute/pool_cpu/ops/analysis/helpers/stat.rs`, `compute/pool_cpu/ops/analysis/helpers/stat_execution.rs`, exact-mode `default/stat_impl.rs` owners, `ops/analysis/histogram.rs`, `image/entropy.rs`, `image/statistics.rs` | Pillow-facing formatted GetExtrema now has `OperationId::GetExtrema`, an exhaustive mode support contract, injected CPU context, Default architecture fallback, and native byte/u16/f32 sample reductions through exact-mode owners. Internal LA16/RGB16/RGBA16/RGB32F/RGBA32F results are CoreOnly typed multiband extrema. `FormattedExtrema` and Python/JS formatters carry these typed results. The separate Rust/JS byte-valued `Image::getextrema()` now has `OperationId::GetExtremaBytes`, exhaustive exact-mode support, injected CPU context, one Default owner per exact mode, and native-carrier scans that preserve the previous byte-result semantics without materializing an RGBA raster. Its byte projections include typed-sample conversion only at the result boundary, raw I/F carrier lanes, and RGB32F's opaque fourth lane. Add operation × mode × architecture parity and the lazy terminal receipt TODO. GetData's prepared dispatch/direct reads, GetBBox dispatch, and formatted GetExtrema dispatch now live in focused helpers so family `ops.rs` stays below the 1,000-line limit. Histogram now has an exhaustive 25-mode support contract, 1/L mask declaration, prepared CPU selection, and materialized-call receipt; exact-mode Default owners scan all byte, I/F, I;16, and CoreOnly native u16/f32 inputs directly into the existing bins. Entropy has its own exhaustive support contract and composes the prepared Histogram child under the same injected CPU context, with a materialized-call receipt. These changes are source-only and unvalidated. Mode-owned band counts also drive histogram allocation, Entropy, GetColors, and multiband Split. Entropy now shares the exact Histogram rules for I/F, I;16, byte bands, and CoreOnly native bins. Stat now has a 25-mode contract, per-mode owners, injected CPU context, specialized I/F paths, and native CoreOnly Histogram composition. Its masked wrapper executes the prepared Histogram child through the selected exact-mode Stat owner using native u32 bins without a widened histogram allocation; lazy receipts remain a source TODO. GetColors has native per-mode counting and typed CoreOnly tuples; expanded 25-mode × Architecture parity and lazy terminal receipts remain source TODOs. Include Pillow mode/value cases and declared CPU route receipts in parity. |
| `image/statistics.rs`, `compute/pool_cpu/ops/analysis/helpers/getcolors_execution.rs`, `compute/pool_cpu/ops/analysis/helpers/stat_execution.rs`, `image/entropy.rs`, `ops/analysis/histogram.rs`, `ops/split.rs`, `ops/quantize/operations.rs` | Split now has a composed operation contract; Quantize now has a `pool_cpu` operation owner and prepared context; mode × Architecture parity and lazy receipts remain pending. GetColors and unmasked Stat now have exhaustive support contracts, exact-mode Default owners, routed CPU context, and native CoreOnly paths; their expanded parity and lazy terminal receipt evidence remain pending. Stat's lazy receipt remains pending; Split child-selection/fallback receipts remain pending; Quantize has an ImageOps contract and context-driven CPU dispatch, with exact L/P/RGB/RGBA routes and native Default owners for all 21 CoreOnly modes. Materialized calls now record requested Backend and exact-mode selection; lazy receipt composition remains pending. Entropy uses its prepared support contract and composed Histogram child. Preserve Stat mask/error ordering, verify Split child routing and output identity, and verify Quantize CoreOnly mode-owner dispatch plus materialized and lazy receipt behavior. Source TODOs and the plan matrix track these gaps; work remains unvalidated. |
| `ops/split.rs`, `compute/pool_cpu/ops/put/helpers/extract_band.rs`, `ops/analysis/bbox.rs` | Split and GetChannel use exact band counts and preserve CoreOnly u16/f32 samples through `ExtractBand`. GetBBox is now a direct CPU analysis operation with a public support descriptor, an exhaustive 25-mode classifier, alpha-band input contract, prepared selection, and one Default owner under each exact mode folder. Pillow `I` and `F` retain their raw-word predicates; each `I;16*` owner scans native u16 samples. Pillow's `GetBBox.c` source comparison confirms that three-band modes ignore padded storage, alpha-only selects only Pillow's five alpha-bearing modes, and internal high-depth modes use their native channel selection. This corrects the archived Rust I;16 byte-prefix scan, which missed samples with only a nonzero high byte. Direct host receipts now carry the selected architecture and fallback reason; lazy-pipeline terminal receipts that combine materialization with the CPU scan remain a TODO. The previous `to_rgba8` fallback and the local row unit test were removed. Pending: CPU parity for all 25 modes under Default and all nine non-Default Architecture requests, verifying same-mode Default fallback, I;16 high-byte cases, and alpha-only selection. This is source-only and unvalidated. |
| `ops/convert/operation.rs` | Separately route matrix Convert through a GPU-owned matrix strategy when GPU declares it; the current typed eager wrapper deliberately executes this CPU-owned operation on CPU while GPU support remains outside the CPU migration. |
| `compute/pool_cpu/ops/composite/ops.rs`, `compute/pool_cpu/ops/draw/ops.rs`, `compute/pool_cpu/ops/paste/ops.rs`, `compute/pool_cpu/ops/transform/ops.rs` | Keep each family source/caller review beside its operation owner: carry Composite's source-reviewed modes, mask pairs, and architecture routes into frozen CPU parity; finish Transform source review; all supported Draw geometry, Text, Bitmap, and native RGB/CMYK fast routes now pass the exact `StrategySelection` into their mode owners; finish Draw mode/error review and frozen parity; carry Paste's source-reviewed SIMD and owner routes into frozen CPU parity. `compute/pool_cpu/ops/mod.rs` only exports operation families. |
| `compute/pool_cpu/ops/geometry/pad/ops.rs` | Move any remaining archived full-width convolution modes into exact mode/architecture Resize owners, and finish checking every padded mode against its same-mode Default fallback and prepared context. |
| `compute/pool_cpu/ops/enhance/ops.rs` | Contrast source review is complete: nine remaining identities reach Blend's preserved wrong-mode rejection, five internal high-depth identities reject while converting the constant base, and La/RGBa retain their exact conversion errors. Sharpness source review traces wrapper→registry→injected context→exact mode→architecture, confirms exact output identity for all 22 accepted modes and archived native behavior for the six Pillow-supported byte modes, and records explicit `NO_PILLOW_SUPPORT` markers on the other 16 native extensions. The six byte-mode architecture lists and dispatches cover all seven declared entries. Keep Sharpness in the frozen CPU parity matrix after source migration is complete. Expand parity across every operation × exact Mode × Architecture. |
| `compute/pool_cpu/ops/filter/ops.rs` | Filter3x3 native-byte Default and architecture kernels are source-compared: coefficient order, arithmetic, border retention, RGB byte-stream, LA binomial path, and exact-mode reconstruction match the archive. Filter5x5 Default paths are source-compared: I32 keeps its typed path; L/LA/RGB/RGBA retain their archived special kernels and bounded fast paths; other admitted byte modes use native-byte rows; each leaf restores its exact variant. I32 also declares the archived AVX/FMA ring for the exact normalized binomial request and retains AVX2/FMA for its vector route. Include guarded exact-kernel selections and ineligible-request Default fallbacks in the frozen CPU parity matrix. P, F32, and internal high-depth rejections remain declared. Direct ModeFilter has an exact byte-mode support contract, normalized window facts, serial CPU context, palette-preserving wrapper, and native exact-variant output; its direct caller was moved out of `ops/param_filters.rs`. Its neighborhood, tie, threshold, P-index, and palette behavior were compared with `3f9dfd403`; the refactored caller records its prepared CPU architecture selection. The support declaration and closed strategy selector now route each accepted byte identity to its exact-mode Default leaf; those leaves call the shared native-byte helper without mode conversion. Mode, palette, and backend-fallback parity remains pending. General Gaussian/BoxBlur source comparison covers the serial and native-byte architecture paths; the RGBA radius-seven implementation and its uniform proof are compared with the archive. L rank-window Default and vector paths are source-compared, including the preserved Apple ARM64 9x9 rank-40 performance regression. Finish comparisons for remaining exact-mode kernel routes and the other direct/eager Filter caller audit. Native-byte extrema/rank strategies and L/F32 rank vector strategies are represented in their exact mode owners. PA BoxBlur/BoxBlurXY and rank requests retain native index/alpha samples so prepared execution reaches the declared PA kernels; GaussianBlur remains rejected for PA. |
| `compute/pool_cpu/ops/resize/ops.rs`, `compute/pool_cpu/ops/resize/{i32,f32}/ops.rs`, `compute/registry/registration/geometry.rs`, `ops/pil_resize/{resize,boxed,luma16,float}.rs` | The prepared path now preserves float32-bound identity/crop shortcuts and unchanged-axis float pass elision; native I;16 Default and vector paths preserve exact output identity for empty source extents. Still review boxed validation/error ordering, and keep the older whole-image dispatcher, boxed adapter, and integer-mode reducers test-only; production CPU execution uses prepared mode owners and shares only low-level coefficient, sample, and pass helpers; move any retained Rayon scheduling to `pool_parallel`; keep the GPU Pillow-string helper as separate migration debt. Public direct and nested geometry callers already enter prepared plans with injected context. All 14 byte modes declare ordinary nearest vector strategies. ImageLuma8 and eleven other byte modes declare unboxed filtered SSE2, NEON, and Wasm SIMD128 strategies when both coefficient axes satisfy the checked i32 bound; One and P remain nearest-only. Pad composes the exact ImageLuma8 strategies into its output window. All byte modes declare boxed nearest strategies, with filtered boxed convolution for all except One/P. I32/F32 filtered Resize and all four unboxed I;16 vector routes are declared with their Default fallbacks; I32 also declares NEON for unboxed nearest. Boxed I;16 remains Default. Thumbnail prepares nested Reduce/Resize work with the same context; its streamed I32 mode owner and AVX FMA adapter now consume the prepared `StrategySelection`. |
| `compute/pool_cpu/ops/eval/ops.rs` | Compare the archived SIMD vector/admission paths against the prepared CPU strategies and migrate any missing mode/architecture case. Move archived Rayon scheduling to `pool_parallel`. Direct scalar point and float-LUT calls enter declared exact-mode CPU strategies and retain their `StrategySelection` in eager receipts. |
| `compute/pool_cpu/mod.rs`, `compute/pool_cpu/batch_execution.rs`, `compute/execution/terminals.rs` | Serial prepared dispatch, routed-context plumbing, wrapper-resolved P/PA palette handoff, ordered PutPixel, transpose and point-LUT runs, Draw, AlphaComposite+Mirror, Multiply+Screen, RGB PutPixel+Thumbnail, and owned Invert/Chops Add/Sub routes are source-reviewed against their mode/architecture guards. Identity transpose and point-LUT runs report `native-identity` without claiming a selected kernel. CPU output/resource/error and selection/fallback parity remains pending, as do separate eager/lazy terminal and nested-context gaps. |
| `compute/mod.rs` | Source-complete: the independent SIMD pool and adapter re-exports are removed. Keep `Backend::Simd` only as a request alias for CPU plus detected Architecture. |
| `compute/execution.rs` | Source-complete: pipeline selection and execution use the injected CPU architecture context; mixed SIMD segments, guessed mode transitions, and retry routing are removed. |
| `compute/automatic_segments.rs` | Removed: CPU mode/architecture strategies now execute the complete prepared operation sequence without a separate segment planner. |
| `compute/eager.rs` (module TODO) | Tiny-RGB ExtractBand now uses the prepared CPU architecture executor for both CPU Default and compatibility SIMD, and records its selection; the generic result-only receipt helper is test-only. PointScalar/PointFloat now honor the image-pinned or active backend when constructing their CPU context, preserve source-graph receipts, and report selected architecture or GPU-to-CPU fallback. Continue auditing remaining direct eager operations for context or prepared-owner bypasses. |
| `compute/pool_cpu/ops/put/ops.rs`, `image/data_access/put.rs`, `image/pixel_access.rs` | Prepared PutPixel/PutData/PutAlpha/PutAlphaData callbacks are consumed by the serial CPU batch path; the contiguous RGB PutPixel+Thumbnail fusion is wired. Immediate PutData callbacks select the exact-mode contract from a routed CPU Architecture context and record a receipt because each write must be visible before the next host value is coerced; current paths fall back to native Default. Numeric I/F, direct I;16 PutPixel, and raw-byte I;16 PutData use exact-mode writers with the shared routed CPU Architecture context and native Default fallback. Finish wrapper metadata and the remaining PutAlpha, PutAlphaData, and ExtractBand conversion reviews. |
| `compute/registry/registration/imageops.rs` | Pad uses the prepared geometry owner. Contain, Cover, Fit, and Scale use the Geometry owner and nested prepared Resize plans; Expand uses its prepared CPU owner. All 25 Pad mode entries now receive the retained `StrategySelection`; specialized owners resolve the exact Pad mode and Default-only owners verify the selected CPU mode. Prepared Resize composition, vertical-append helpers, and direct Resize-to-canvas comparison receive the same selection. Static source review only; native component/error semantics, P palette mutation order, archived kernel comparison, and direct/eager caller review remain TODOs. |
| `compute/registry/registration/simd.rs` | Removed from the active tree. Compare all archived adapter groups with the assigned CPU mode/architecture owners and migrate any missing strategy or fallback. |
| `compute/pool_cpu/ops/crop/ops.rs` | Expand parity across every exact Mode × available Architecture, including archived SIMD admissions, Default fallbacks, I;16 vector blocks/tails, and CropBorder empty/error contracts. The archived source comparison is complete; the ordinary byte row-copy algorithm is the mode-owned Default `ByteCrop`. |
| `compute/pool_cpu/ops/color3dlut/ops.rs` | Expand parity across Pillow source modes, target modes, channel counts, and available architectures, including native-source core-only modes, the eight-pixel Default fallback, vector tails, and rejected LUT shapes. The archived CPU/SIMD comparison is complete; active Default strategies consume native sample carriers. |
| `compute/pool_cpu/ops/imageops/ops.rs` | The family entrypoint owns Autocontrast, Equalize, Expand, and Quantize support descriptors and the Autocontrast, Equalize, and Expand pipeline front doors; mode classifiers and prepared native strategies remain in their operation modules. Quantize now passes its retained `StrategySelection` through all 25 exact-mode owners, including macro-generated CoreOnly routes and supplied-palette L/RGB/PA paths. Finish direct/fusion caller reconciliation for Invert, Posterize, Solarize, Equalize, Autocontrast, Colorize, CropBorder, and Expand; compare remaining Invert, histogram, and Expand routes. The public Autocontrast/Equalize/Posterize/Solarize mode checks now use their operation declarations, while P Posterize/Solarize remain CoreOnly native-index strategies. Colorize supports Pillow L only and does not project other modes through L. Invert's P/LA/PA/RGBA/CMYK routes are declared CoreOnly; source-aware materialization preserves native P/PA strategies and palette metadata. Target-only parity remains pending. |
| `compute/pool_cpu/ops/imageops/ops.rs` (`prepare_crop_border`) | Expand parity for CropBorder's outer and composed Crop selections across exact modes and architectures, including I;16 guard fallbacks and empty outputs. Source review confirms width-before-height errors and the archived ordinary-byte admission. |
| `ops/imageops/geometry.rs` | P-mode Expand tuple/name resolution consumes the exact-mode `OperationId::Expand` `PaletteIndexOrColor` declaration; palette allocation, error order, and output metadata parity remain pending. |
| `compute/pool_cpu/ops/transpose/ops.rs` | Native-byte transpose, including tiled/gathered axis swaps and same-dimension methods, is declared for the archived native-byte and I;16 identities under SSE2, NEON, and Wasm SIMD128; RGBA AVX2 remains preferred for eligible axis swaps. Native-byte guards admit the archived logical source prefix and exact empty storage; I;16 guards keep empty storage on Default. Parity must verify those boundaries and architecture receipts. The archived byte Flip adapter had no distinct vector kernel, while I;16 Flip retains its vector owner. Keep archived parallel collection kernels for the later `pool_parallel` stage. |
| `compute/pool_cpu/ops/thumbnail/ops.rs` | The Thumbnail operation contract now links conditional Reduce, Resize, and ResizeBoxed support, and the prepared receipt records the exact selected child strategies from the injected context. Continue the archived vector Thumbnail mode/kernel comparison and retire compatibility callers after source review. The streamed ImageI32 2x2 path now has exact Default and guarded AVX/FMA owners. Mode-specific nearest-filter rules for `1`/`P`, reducing-gap exceptions for straight-alpha `LA`/`RGBA`, stored-sample reduction for `La`/`PA`, and the F32 unboxed Resize policy now live in mode-owned strategy folders. |
| `compute/pool_cpu/ops/rotate/ops.rs` | The maintained owner declares all 25 exact modes with serial Default strategies, including typed native `u16`/`f32` paths for the five internal identities, each marked `NO_PILLOW_SUPPORT`. The serial three-byte contiguous-copy and RGBA unrolled nearest paths are restored. All byte-backed exact modes declare native-byte nearest strategies for AVX2, SSE2, NEON, and Wasm SIMD128, including raw-byte `I`/`F` carriers; byte modes declare their archived bilinear vector kernel where supported, including typed I/F arithmetic. Architecture-selected identity copies and exact right-angle transposes now use native vector-copy/transpose helpers and retain their receipts for eligible nonempty inputs. Empty-source Rotate requests now use the exact mode's serial Default strategy; byte-backed and native typed Default paths permit empty output only for archived 180° and expanded 90°/270° fast paths, while other empty geometry retains Default's checked behavior. The four I;16 identities remain typed-u16 Default-only. Source comparison confirms the archived raw-byte SIMD nearest route uses signed 16.16 coordinate stepping and byte-plane sampling, which differs from the maintained Pillow I;16 path's floating Geometry.c mapping over typed u16 samples; the mode owner records a TODO for a future exact-geometry typed strategy rather than declaring that incompatible route. Remaining work is per-mode direct/compatibility caller review plus CPU parity, including empty-output and fallback receipts; core-only extensions stay outside Pillow parity. |
| `compute/registry/registration/imageops.rs` | Source-review direct and eager ImageOps callers against the prepared CPU registry; grayscale's bounded RGB eager path already uses the prepared plan and injected routed context. |
| `image/materialization.rs` | Keep typed prepared plans flowing through the serial pool and preserve resolved operation dependencies on wrapper-to-pool routes. Single-operation lazy P/PA Grayscale, Equalize, and Invert use their declared exact-mode CPU strategies before generic palette expansion. Other CPU indexed requests validate the first `PipelineOp::support()` mode declaration against the materialized P/PA source before expansion, so direct `Image::push_op` cannot dispatch a rejected request as RGB(A). Finish GPU palette-strategy separation in its later backend stage. |
| `image/pipeline_ops.rs`, `pipeline/support.rs`, `compute/mod.rs`, `lib.rs` | `PipelineOp::operation_id()` and `PipelineOp::support()` now expose the same typed operation declarations used by preparation; Convert selects the matrix contract when its matrix is present, and draw/chops variants map to their owning operation identities. `OperationId`, support descriptor types, `ExecutionContext`, and `Architecture` are re-exported at the crate root for Rust callers. Autocontrast, Equalize, Posterize, and Solarize wrappers now validate modes through their operation declarations; Filter and GaussianBlur already use the declared support path. Native internal high-depth Posterize/Solarize rejection reasons now match their public mode labels. The Rust `blend` wrapper validates mode pairs and dimensions through `BlendModule`'s `InputContract`, checking mode pairs before dimensions to preserve error order; it retains an explicit backend from the first operand, or inherits the second operand's pin when the first is an eagerly materialized base. The materializer rejects direct P/PA BlendModule requests from that same declaration before palette expansion. Python Color's eager-base/public-Blend caller is source-traced; the new context handoff remains unverified. Verify Brightness float32 byte blending and architecture-fallback receipts in parity, and include Color's host path and metadata behavior in the frozen parity scope. Retain Contrast's documented conversion-first behavior and Sharpness's completed exact-mode route. Preserve exact rejection/error order, then remove the source-mode fallback only when every accepted P/PA transition has a declared result. |
| `compute/registry/support.rs`, `compute/registry/support/operation.rs`, `raster/mode.rs` | Keep `support.rs` as the shared contract facade and re-export `OperationId` from its size-bounded family mapping submodule. `Mode::uppercase_a_band_index()` exhaustively identifies the uppercase `A` band across all 25 mode identities; ColorSaturation and Contrast consume this typed mode fact. |
| `image/construction.rs`, `image/mode_boundary.rs`, `image/pixel_access.rs` | Convert remaining core string-taking constructors, generated-image and pixel-mutation entry points to typed `Mode`; keep Pillow string parsing at host/API boundaries. |
| `image.rs` | Remove the remaining fallback mode identity and reconcile exact materialized and cached pipeline mode ownership. |
| `image/codec_adapter.rs`, `image/metadata.rs` | Header projection now maps every current codec `ImageMode` to the exact raster `Mode`, including internal LA16/RGB16/RGBA16 and float identities. After decoding, `DynamicImage` remains authoritative. Review format-specific header metadata and verify at the CPU gate. |
| `ops/filter.rs` | Filter validation obtains exact `Mode` through `Image::mode_identity()`. The direct ModeFilter preflight uses its operation support descriptor; UnsharpMask, palette, PA GaussianBlur, and F32 compatibility gates remain at this host-facing boundary. Reconcile their outcomes with prepared contracts while preserving lazy error timing, then finish the Pillow filter-name/mode/error and direct-caller review. |
| `compute/pool_cpu/ops/filter/ops.rs`, `compute/pool_cpu/ops/filter/luma8/ops.rs` | Native-byte Filter3x3, general Gaussian/BoxBlur paths, RGBA radius-seven GaussianBlur, and L rank-window paths have source comparisons recorded. Continue comparison of remaining exact-mode kernel routes; reconcile direct/eager Filter callers and finish archived mode/kernel review. Filter5x5 Default paths have been source-compared. No separate SIMD registrations remain active. F32 rank vector strategy is represented in `f32/ops.rs`. |
| `image/metadata.rs::Image::mode_identity` | The ImageOps point and module point/eval wrappers now delegate to the shared resolver. Remove its materialization fallback after all lazy pipelines carry an exact cached `Mode` through admission and transitions. |
| `ops/param_filters.rs` | Keep parameter-filter wrappers as argument normalization and result/metadata orchestration; direct ModeFilter must enter its typed support contract and injected CPU context in `pool_cpu/ops/filter`. |
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
- `EffectSpread` and `EffectNoise` public calls enqueue typed `PipelineOp`s, the active registry selects their prepared CPU family callbacks, and `PreparedOperation` executes the selected exact-mode strategy. Their caller wiring is source-traced. The former SIMD operation registrations are removed from the active registry. Source comparison confirmed that the archived Default path returns a native clone and advances the process RNG for nonempty distance-one `L`, `LA`, `RGB`, and `RGBA`; only these four mode owners now opt into the shared identity shortcut, avoiding an unnecessary relocation map while retaining the baseline routing boundary. The archived SIMD adapter also admits distance one; its maintained byte-layout guard keeps that architecture selection and the kernel advances the same process RNG. Remaining TODO in `pool_cpu/ops/effects/ops.rs`: compare the other retained architecture guards and kernels against the archived SIMD adapter, then verify exact-mode selection and same-mode Default fallback in CPU parity. This family and CPU backend remain incomplete and unvalidated.
- EffectSpread source comparison: the shared native SIMD data plane retains ordered source traversal, immutable-input reads for both exchanged pixels, the archive's modulo and displacement arithmetic, 16-byte pixel-work receipts, and native mode reconstruction. Its Default relocation helper preserves the archive's early empty/distance-zero exits, checked distance/width/height order, and RNG draws. For distance one, the four archived Pillow-mode Default fast paths now clone the native buffer and jump the same two random draws per pixel; every other mode remains on its declared native sample strategy. Source-only; pixel and receipt parity remain pending.
- EffectNoise's Default RNG/control path matches the archived polar Box-Muller sequence, `f32` sigma narrowing, clamp/truncate conversion, and one discarded deviate per output pixel. The shared vector path batches only the final `128 + sigma * deviate` arithmetic and consumes random pairs in scalar pixel order, including its final partial lane group. EffectNoise still writes L from source dimensions. Source-only; CPU mode × Architecture parity and selection receipts remain pending.
- Generator source comparison: LinearGradient retains the archive's exact five-mode contract (`1`, `L`, `P`, `I`, `F`), logical sample values, typed I/F sample representations, and default error text; `1` uses the agreed expanded-byte raster carrier, and vector routes cover the same five identities. RadialGradient keeps the same five outputs and scalar distance formula, with the archived SIMD implementation restricted to L. Mandelbrot keeps the zero-size early return before extent/quality validation, the same `unrecognized argument value` rejection, default recurrence order, and eight-lane vector threshold; current constructor entry points inject the routed CPU architecture before execution. Source review is complete for Effects; runtime parity, mode/architecture receipts, and fallback outcomes remain TODO.
- No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage command has run.

### Rotate-family source progress: source review complete; runtime unvalidated

- The new `rotate` owner declares all 25 exact mode strategies and returns the exact corresponding `DynamicImage` identity. Its entry point owns request facts and operation support; each mode's `ops.rs` declares its support and binds a Default implementation. The five internal native high-depth strategies use typed samples and carry nearby `NO_PILLOW_SUPPORT` comments.
- Shared geometry source comparison (2026-11-01): `helpers/geometry.rs` preserves the archived reverse-affine coefficient order, center/translation composition, rounded coefficients, ordinary floating-point corner bounds, independent ceil/floor output edges, and centered expanded-canvas shift. The public wrapper normalizes angles before queuing, matching the archived `rem_euclid` input to the affine helper. This closes the shared geometry source slice only; it does not verify per-mode interpolation, architecture receipts, or parity.
- Pillow-defined `1`/`P` retain forced nearest sampling, `LA`/`RGBA` retain straight-alpha premultiplication around filtered sampling, and `La`/`RGBa`/`PA`/`RGBX`/CMYK retain raw stored-channel semantics. I/F filtered Default paths and architecture-selected bilinear paths treat their four-byte carriers as typed scalar samples; nearest paths preserve the raw carrier bytes. I;16 variants retain their endian-specific native storage, and I;16 nearest sampling keeps its archived floating-point Geometry.c coordinate mapping.
- The serial byte nearest kernel restores the three-byte contiguous source-span copy and RGBA four-pixel unroll from the archived CPU implementation. Exact-angle and unexpanded rectangular right-angle routing follows the archived behavior. I;16 filtering retains the archived low-byte filter arithmetic without casting the raster mode to L.
- All byte-backed exact modes now declare AVX2, SSE2, NEON, and Wasm SIMD128 strategies in their exact mode architecture folders, including `LA`, `La`, `PA`, `RGBA`, `RGBa`, `RGBX`, `CMYK`, `YCbCr`, `LAB`, `HSV`, and the raw-byte `I`/`F` carriers. With exact native bytes and finite geometry, guards admit the declared identity/right-angle, nearest, or bilinear kernels; unsupported requests use the same-mode Default strategy. Empty-source requests always use Default so its mode-specific fill and allocation behavior stays consistent. `One` and `P` retain forced-nearest behavior, and each result keeps its exact mode identity. The vector kernel gathers native samples into 16-byte blocks and retains scalar tails; three-byte modes also retain contiguous source-run copies. The SSE2 build declaration is disabled when AVX2 is a compile-time target feature, so an SSE2 context cannot reach code built for a stronger ISA.
- The same mode/architecture owners declare the archived eight-lane bilinear kernel for eligible byte modes. Its interpolation and geometry helpers stay in Rotate helpers; `LA` and `RGBA` select the alpha-aware path, while premultiplied/indexed/padded/color-space modes preserve their stored channel bytes. `I`/`F` use their separate typed bilinear vector kernel and retain raw-byte nearest sampling. Default I/F bilinear interpolation is const-specialized by exact mode, removing the runtime numeric-type branch from each sample. The architecture identity shortcut matches Default's epsilon/no-center/no-translation contract, preventing explicit transform parameters from bypassing alpha-aware filtering. Empty-output behavior and architecture selection receipts remain in the frozen CPU parity matrix.
- Archived SIMD admission also included `ImageLuma16` for nearest transforms with nonempty output, exposed through all four `I;16` mode strings and a two-byte native layout. The active exact-mode `ImageI16`, `ImageI16L`, `ImageI16B`, and `ImageI16N` owners currently declare only Default. Their Default owner reads typed u16 storage and reproduces the archived Geometry.c floating coordinate mapping; the archived SIMD kernel instead advances signed 16.16 coordinates and gathers raw byte pairs. Do not route those modes to that kernel until coordinate-boundary behavior, per-mode endian byte order, fill bytes, and typed output reconstruction are reconciled. Tracked TODO `pillow-rs#3ptb`; no runtime evidence collected.
- `Image::rotate` and its typed-input wrapper enqueue `PipelineOp::Rotate`; the registry maps that operation to this family's `prepare_pipeline`, and the serial CPU batch executor calls it through `prepare_cpu` with the routed `ExecutionContext`. Ordinary lazy materialization, palette-safe P/PA execution, and cached prefix/suffix execution all prepare with the routed context; mode-preserving metadata and each mode owner retain the exact `DynamicImage` identity. No eager Rotate bypass or separate maintained `op_rotate` implementation is active. The Python dirty-palette path prepares once before palette realization, then verifies the captured exact mode before enqueueing; ordinary Python and direct Rust calls use the same lazy operation path. `PreparedOperation::execute` and the regular batch path record the selected `StrategySelection`, including fallback reason, before executing the selected leaf. This closes the compatibility-route and receipt source audit; runtime receipts remain pending.
- Public Rotate wrapper comparison against the baseline found the same validation order for non-finite expanded angles, resample normalization, indexed-mode nearest forcing, fill conversion, then center/translation validation. The wrapper now resolves the Pillow-owned enum `Mode` for the indexed/alpha-fill choices and places the effective nearest filter on the queued operation; direct Rust `Image::rotate` uses the same queue path. Source review maps the exact-mode kernels and flags: `1`/`P` force nearest; `LA`/`RGBA` use alpha-aware filtering; `La`/`RGBa`/`PA`/`RGBX`/CMYK/YCbCr/LAB/HSV retain raw stored-channel behavior; `I`/`F` use typed scalar interpolation; I;16 variants use native-u16 Default; and internal high-depth identities use native samples with `NO_PILLOW_SUPPORT`. All declared byte-mode architecture routes use the exact owner's channel count, forced-nearest, and alpha policy, then reconstruct the same exact identity. The I;16 SIMD adapter remains undeclared because its signed 16.16 coordinates and raw byte fill do not preserve Pillow's typed-u16 Default contract. Per-mode output, empty-output, and architecture receipt parity remain open.
- Existing Rotate regression tests are not the migration's verification plan and have not been run. Do not add unit tests for this refactor; the planned verification is parity across each applicable operation × exact Mode × Architecture combination, including vector tails, Default fallbacks, and rejected requests. This family remains source-only and runtime-unvalidated; no build, formatter, checkstyle, lint, test, parity, benchmark, or coverage command has run.

### Registry request-support source progress: incomplete and unvalidated

- Split semantic `InputContract` assessment into `registry/support/request/contracts.rs` and materialized `StrategyGuard` checks into `registry/support/request/guards.rs`. The parent module now delegates to these focused helpers and stays below the 1,000-line file limit. Error classes, messages, unresolved-input results, and exact byte-layout conditions were carried over during source review.
- Split the oversized materialized guard implementation into `guards/core.rs` and `guards/mutation.rs`, leaving `guards.rs` as a small export module. The 39 guard declarations retain their existing names and are all re-exported through the path consumed by `StrategyGuard::assess`; each maintained file is below 1,000 lines. This is source-only and uncompiled.
- Pipeline strategy-receipt audit (2026-11-02): the standard CPU batch opens an operation receipt before preparation, and `PreparedOperation::execute` sends its full `StrategySelection` to `record_pipeline_operation_selection` before running the selected leaf. That recorder stores both the selected Architecture and each unique `FallbackReason`; PyO3 and JavaScript telemetry adapters expose the reason names. The inspected fused and owned-special batch routes also call the same recorder. The earlier note that the normal batch path dropped strategy fallback reasons was stale and is corrected here. Remaining direct/eager/terminal composition paths are tracked separately; frozen parity must still exercise MissingSpecialization, UnavailableBuild, UnavailableHardware, and IneligibleRequest receipts. No runtime evidence collected.
- The support source review is not complete for the repository as a whole; remaining functions and files still need the planned size review after the implementation migration. No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage command has run.

### Put-family source progress: incomplete and unvalidated

- PutAlpha and PutAlphaData now have native typed paths for `I;16*`, internal 16-bit luma/RGB/RGBA, and internal 32-bit float RGB/RGBA. These paths preserve source color samples and emit the declared LA16, RGBA16, or RGBA32F output; they do not route through an 8-bit RGBA carrier. The u8 alpha sample is expanded to the full u16 range or normalized to f32 at the destination sample type.
- Conversion review removed three copy-only source clones from the L constant-alpha, L image-mask, and LA image-mask Default leaves; their helpers borrow the source while allocating the required output carrier. Both CMYK Default alpha leaves now consume `cmyk_buffer_to_rgb` directly instead of wrapping the typed RGB result in `DynamicImage` and cloning it back with `to_rgb8`. The CMYK arithmetic and RGBA output samples are unchanged. PutAlphaData accepts exact One/L masks and consumes their shared one-byte sample carrier without constructing an intermediate L image. This is source-only and unvalidated.
- The `I` and `F` PutAlpha modes remain explicitly rejected with their existing Pillow error messages. The new I;16 alpha outputs are core-only extensions and carry nearby `NO_PILLOW_SUPPORT` comments.
- PutAlphaData now preserves accepted One/L masks through the pipeline and reads their native one-byte luma samples directly. Its operation contract lists exactly those accepted mask modes; unsupported modes still return `illegal image mode`. Admission rejects mismatched image and mask dimensions before the pixel loop, preserving the existing `images do not match` error instead of silently truncating zipped rows.
- PutData, PutAlpha, PutAlphaData, and ExtractBand cross-mode conversion reviews are complete at source level. PutAlpha's declared pipeline output now shares the lazy-planner transition mapping. Other wrapper transitions, full family/caller review, and CPU migration remain in progress. The current mode-cache and exact-mask propagation changes are source-only and unvalidated. No build, formatting, checkstyle, lint, test, parity, benchmark, or coverage command has run; do not treat the family or CPU backend as complete.

### Filter-family migration status: in progress and unvalidated

- The maintained `pool_cpu/ops/filter` family has an operation entry point and per-mode support and execution modules. Filter3x3/5x5, byte blur, and byte rank now dispatch through exact-mode Default leaves; blur/rank helpers receive native bytes and dimensions from those leaves and do not inspect `DynamicImage` or `Mode`. The shared `filter/ops.rs` entry point handles admission and exact mode selection only.
- Kernel mode support source correction (2026-11-03): Pillow 12.2's `BuiltinFilter` rejects `P` before calling `ImagingFilter`; the C entry accepts UINT8 modes and INT32, accepts single-band SPECIAL I;16, and rejects FLOAT32 and multiband SPECIAL. The exact-mode declarations now include the 13 admitted byte identities (including `One`), `I`, and all four I;16 identities, while keeping `P`, `F`, and internal multiband high-depth identities rejected. `I;16`, `I;16L`, `I;16B`, and `I;16N` each have a Default owner under their exact mode folder; these owners call `filter/helpers/i16_kernel.rs` directly on their native u16 samples, preserve their exact DynamicImage variants, and share no converted raster. The helper follows Pillow's bottom-to-top coefficient order, offset bias and ties-away rounding, separately clips the two output bytes, and copies borders. Rust safely defines Pillow's C float-to-int undefined cases; a TODO under `pillow-rs#3ptb` tracks non-finite and out-of-range I;16 results and NaN I results from `clip32`. Sources: [`ImageFilter.py`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageFilter.py#L42-L56), [`Filter.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Filter.c#L25-L47). This remains source-only and parity-unverified.
- Native I32 3x3/5x5 and rank implementations, plus F32 rank, have been moved into their typed mode owners. The retained I32 5x5 vector kernel is under `filter/i32/avx2`; its support declaration binds `Filter5x5` to AVX2+FMA build and host capability selection. The archived serial scalar-FMA ring is also under `filter/i32/avx`, selected only for the exact normalized binomial request when AVX+FMA is available; other requests fall back to same-mode Default.
- The retained native LA 5x5 binomial and single-outlier vector kernels are now under `filter/luma_a8/helpers`, outside the NEON folder because their packed `wide` implementation is shared by vector architectures. The LA mode strategy checks those exact requests before its generic native-byte vector loop; the dedicated NEON orchestration remains in `filter/luma_a8/neon`, and unmatched kernels use the same mode's Default strategy.
- The retained LA 3x3 binomial NEON kernel is now under `filter/luma_a8/neon`; its architecture strategy requires the exact request, native two-byte layout, and at least one full vector block, with same-mode Default fallback.
- The retained I32 3x3 four-lane convolution is now under `filter/i32/avx2`, next to I32 5x5. Its AVX2/FMA strategy requires finite coefficients, nonzero finite scale, exact four-byte storage, and an interior vector block; other requests use the same-mode Default path.
- The archived native L 5x5 binomial and uniform integer vector kernels are now under `filter/luma8/helpers`, outside an architecture folder because their `wide` implementation is shared by vector architectures. The L mode strategy invokes them only for those exact requests and the exact one-byte L variant; unmatched kernels continue through the generic vector loop, and Default selection keeps the serial implementation.
- The two Filter unit-test source references that called `pool_simd::ops::native_filter` now point to the mode-owned CPU architecture kernels. No tests or other validation have run.
- `Image::validate_filter` obtains typed `Mode` through the shared resolver; its ModeFilter, UnsharpMask, palette, PA GaussianBlur, and F32 early-rejection gates resolve known filter names through the same `OperationId` mode-support declarations used by prepared operations. Unknown palette/F32 names retain their previous boundary errors, while other modes defer rejection to prepared execution. Source review traced the public Python filter call: it instantiates the callable filter, resolves `.name` or its type name, validates mode, checks `_apply`, then invokes it. The built-in `Image::filter` validates before name dispatch; `kernel_filter` validates mode before kernel shape and coefficient checks; parameterized blur, UnsharpMask, ModeFilter, and rank methods validate mode before their parameter checks or eager execution. This preserves the existing validation-before-`_apply` and mode-before-parameter ordering by source inspection; parity remains pending. UnsharpMask prepares its exact-mode blend and nested GaussianBlur from one injected CPU `ExecutionContext`, executes the composition through the eager CPU strategy receipt, and returns the same mode-bearing raster. Its operation contract exposes the exact radius, percent, and threshold facts. Filter3x3, Filter5x5, GaussianBlur, BoxBlur, and rank operations expose typed request parameters alongside their mode support. The RGB 150% portable-wide kernel is under `filter/rgb8/helpers/unsharp_blend.rs`; accepted byte modes use the shared same-mode byte blend as Default. The previously unreachable malformed-storage byte fallback was removed after exact mode admission made the native layout authoritative. Color3DLUT's eager source admission now uses its operation-level mode declaration; the operation support view exposes the same target-mode/channel contract used by prepared strategy guards, and the Image API resolves target channel capacity from that declaration. Source-only; no compile or parity evidence exists.
- `registry/registration/filter.rs` points at the prepared CPU family owner and retains the separate GPU callbacks. Built-in/rank operations enqueue typed `PipelineOp`s; ModeFilter and UnsharpMask's eager public routes both construct a routed CPU `ExecutionContext` and execute their prepared mode strategies through the eager receipt path. Caller wiring is source-traced; include both eager paths and GPU fallback receipts in the frozen parity matrix.
- Filter mode support and execution entry points are visible to the owning `filter` family only, allowing its family-level orchestrator to compose sibling mode strategies without widening them to the crate API. The RGB Unsharp and mode-owned blur entry points follow the same boundary. Source-only; no compile check has run.
- Public Filter caller source review confirms Python `Image.filter` instantiates callable filter classes, resolves `.name` or the type name, calls Rust `validate_filter`, checks for `_apply`, and invokes the method in that order. Built-in and rank wrappers queue typed `PipelineOp`s after mode preflight, while ModeFilter and UnsharpMask are eager; `kernel_filter` validates mode before kernel shape and coefficient checks. Known early-rejection names use the operation's support declaration, and unknown P/F names preserve their previous mode error. Parameterized blur, UnsharpMask, ModeFilter, and rank entry points retain mode-before-parameter/eager-execution order by source inspection. Exact public error-order parity remains pending; see the operation-owned TODO in `filter/ops.rs`.
- The exact 3x3 binomial request recognizer is shared from `compute/helpers/filters` by CPU Filter3x3, its LA NEON strategy guard, and GPU admission, so backend support checks no longer reach into the CPU operation tree for this common request fact.
- All 13 admitted byte modes execute Filter3x3 through exact-mode Default leaves; P remains explicitly rejected, while I32 keeps its native scalar implementation and F32 remains rejected. Their shared raw-byte helper receives the declared channel count and never infers source mode or output identity. Each leaf reconstructs its own exact DynamicImage variant. The archived generic native-byte vector kernel is now a shared family helper selected by each exact byte mode's declared architecture strategies (SSE2, AVX, AVX/FMA, AVX2, AVX2/FMA, NEON, and Wasm SIMD128) when those builds are available. It preserves the RGB byte-stream kernel, grouped LA/RGBA/CMYK lanes, and the bounded small-uniform control path; LA retains its exact binomial NEON and non-NEON vector fast paths.
- The archived native-byte Filter5x5 vector loop is also shared across all 13 admitted byte modes under their exact mode-owned architecture declarations. The typed vector leaves retain a bounded uniform identity proof and native copy, RGB's large-uniform proof scan and BLUR-ring vector kernel, L's portable wide uniform/binomial kernels, and LA's packed-vector single-outlier/binomial kernels with its dedicated NEON strategy. Non-identity uniform inputs continue through the selected vector strategy; RGBA's exact uniform-kernel fast path remains in its Default leaf. `NO_PILLOW_SUPPORT` remains attached to native extensions; P remains rejected, and I32 retains its typed AVX2/FMA vector route plus the guarded AVX/FMA scalar ring. This source-only migration has not been compiled or verified.
- Filter scope now distinguishes Pillow-defined routes from native extensions: Pillow's Kernel contract is L/RGB, BoxBlur and GaussianBlur use Pillow's supported uint8 mode set (L, LA, La, RGB, RGBA, RGBa, RGBX, CMYK), and rank filters use scalar modes (1, L, I, F). Other admitted strategies use `ModeSupport::cpu_core_only` with nearby `NO_PILLOW_SUPPORT` comments; rejected modes remain rejected. This contract classification is source-only and unverified.
- The archived L radius-one BoxBlur and L/LA/RGB GaussianBlur specializations remain under their exact mode owners. A new `FilterNativeByteBlur` guard exposes the general BoxBlur, BoxBlurXY, and GaussianBlur vector strategy for the exact L/LA/RGB/RGBA/RGBa/RGBX mode declarations that the legacy native-byte adapter admitted. The shared helper preserves the eight-lane fixed-point rolling recurrence, replicated edges, separate horizontal/vertical passes, no-op vector-copy admission, and same-mode Default fallback; it contains no Rayon scheduling. L, LA, and RGB keep their radius-one kernels, while other eligible radii use the shared recurrence. The mode and architecture selection is retained through execution context checks. Source-only and unverified.
- Direct `Image::mode_filter` has moved from `ops/param_filters.rs` into the Filter owner. Its operation support uses exact `DynamicImage::mode()`, zero-as-identity or a normalized positive odd-window fact, serial Default selection, and only native byte identities; I/F and high-depth identities reject before byte processing. The helper retains strict lower-value tie breaking, the `max_count > 2` replacement rule, skipped out-of-bounds neighbors, per-channel byte processing, and same-mode output construction. The wrapper retains eager behavior and palette metadata while routing CPU/GPU/compatibility requests through injected context and CPU fallback telemetry. Source review only; no build, parity, or performance evidence exists.
- ModeFilter source comparison against baseline `3f9dfd403`: the direct eager wrapper still preserves zero/one as the identity window and rounds larger even sizes up to the next odd size. Its exact-mode prepared owner selects the native typed byte carrier, counts neighbors in row-major order while skipping out-of-bounds coordinates, uses strict-greater histogram selection so ties choose the lower sample, and keeps the original sample unless the winning count exceeds two. P output remains `ImagePalette8` and the wrapper retains its palette metadata; all other accepted byte modes return their same exact `DynamicImage` identity. This closes the ModeFilter algorithm/source slice only; wrapper coercion and runtime receipt parity remain pending.
- RGBA GaussianBlur source review now compares the radius-seven route with the archived SIMD adapter: both use the three fixed-point horizontal passes followed by three vertical passes, the exact native RGBA layout, radius seven, and nonzero fractional weights. The 256×256 uniform proof keeps the archived constant-pass arithmetic, probe grid, full wide-vector scan, and scalar tail; it runs before the kernel. AVX2 keeps its target-feature middle-block kernel and chooses the prepared AVX2 transpose when the transpose guard admits the shape, with the declared transpose fallback for smaller tiles. NEON retains the archived row-major vertical kernel only for shapes at least 64×64; smaller shapes reuse its NEON horizontal kernel around the injected-context transpose route, matching the archived transposed pass schedule. The radius-seven guard now admits every nonempty native RGBA shape so those smaller architecture paths remain reachable. Source review only; no build, parity, or performance evidence exists.
- General GaussianBlur, BoxBlur, and BoxBlurXY source comparison is complete for the active serial and native-byte architecture paths. The fixed-point parameter helper retains the archived f32 weight and f64 Gaussian-radius arithmetic. Serial Default preserves no-op and bounded uniform checks, L/LA/RGB/RGBA radius-one specializations, three-pass horizontal fusion, horizontal-before-vertical order, byte rounding between passes, and the same-mode fallback. The shared vector line kernel retains the archived eight-lane `u32x8` rolling recurrence, leading/middle/trailing/clamped sample indices, fractional edge contribution, 24-bit bias/shift, and same transposed vertical pass order; the archived general transpose was serial, while RGBA radius-seven AVX2/NEON transposes remain in their mode architecture owners above. L/LA/RGB radius-one vector specializations remain in their exact mode helpers. Serial Rayon variants were removed from the maintained CPU helpers. This is source review only; no build, parity, or performance evidence exists.
- The archived L 3x3 order-statistic kernel now lives under `filter/luma8/helpers/rank3x3.rs`; MedianFilter(size=3), RankFilter(size=3, rank=1), and RankFilter(size=3, rank=4) select it only for exact native L bytes and nonempty images. Other odd L rank windows through 15x15 use `filter/luma8/helpers/rank_window.rs`; larger windows retain same-mode Default. Both architecture helpers preserve the bounded small-uniform identity path, and mode orchestration matches the exact L raster once before calling typed helpers. The comparison network, second-smallest selector, general rank selection, replicated-edge gathers, and output identity were reviewed against the archived L SIMD path; this source-only migration is unverified.
- The archived F32 8-lane order-statistic kernel now lives at `filter/f32/helpers/rank_vector.rs`. The F32 MedianFilter, MaxFilter, MinFilter, and RankFilter declarations select it for exact four-byte F storage, nonempty images, odd windows through 9x9, valid ranks, and order-compatible samples; other windows retain F32 Default. The shared sample classifier lives at `filter/f32/helpers/rank_order.rs` and is used by Default and architecture preparation. Prepared non-Default requests capture `F32RankCompatibility` from the exact F32 bytes, and the same declared guard defers when that fact is absent or selects Default when NaNs or mixed signed-zero samples require Pillow's serial comparison order. The Default owner now uses a safe Rust implementation of Pillow 12.2's in-place rank partition; it reads and writes native F samples and reconstructs the exact ImageF32 identity. The shared maximum window constant keeps fact preparation, the guard, and the vector kernel aligned. The exact-bit uniform proof is shared by F32 Default and the architecture strategy. The vector window gather, order-statistic sorting, edge replication, uniform identity path, and telemetry were reviewed against the archived SIMD adapter; this source-only migration is unverified.
- Source reconciliation of native-byte MaxFilter/MinFilter and shared MedianFilter/RankFilter architecture paths is complete. Extrema retains the archived 16-lane separable clamped min/max passes, 64x64 uniform and 256x256 all-zero copy guards, output identity, and vector receipt. Rank retains the 225-sample bound, odd-window/rank admission, 3x3 L/LA/RGB specializations, generic odd-even sorting through 5x5, byte-domain selection through 15x15, clamped gathers, and exact mode reconstruction; L remains routed through its dedicated helpers. These comparisons are source-only; they do not constitute parity evidence. F32 Default now mirrors Pillow's in-place comparison/partition order; the content guard keeps NaNs and mixed signed-zero samples on Default because vector sorting may choose different output bits. Add exact NaN and signed-zero parity cases across F32 rank operations and architectures before claiming parity (`pillow-rs#brg0`).
- Remaining family work: complete per-operation support/input/output contracts and confirm every declared native extension has a nearby `NO_PILLOW_SUPPORT` comment; compare remaining rank mode paths; include the source-traced direct/eager routes in parity. ModeFilter's per-mode source algorithm and output identities are compared against baseline above. Filter5x5 Default source comparison confirms the archived allocation order and arithmetic path: the bounded identity proof precedes general rows, L/LA/RGB/RGBA retain their mode-specific fast paths, and other admitted byte modes retain the shared native-byte loop. I32 remains typed and unclipped; rejected modes remain rejected. Its archived scalar AVX/FMA ring is now a declared exact-binomial strategy alongside the existing AVX2/FMA vector route; both paths remain unverified. The archived native-byte MaxFilter/MinFilter and non-L byte MedianFilter/RankFilter kernels now have exact-mode architecture declarations, native-layout/window admission, and same-mode output construction in the maintained Filter family; these source changes remain unverified. UnsharpMask's nested GaussianBlur now uses the same injected context and the mode-aware prepared filter owner. The maintained architecture strategies now include shared byte Filter3x3/Filter5x5 vector kernels, RGB's Filter5x5 BLUR-ring kernel, the packed LA binomial 3x3 vector fast path for non-NEON architectures plus its dedicated NEON strategy, portable wide L uniform/binomial Filter5x5 kernels, packed-vector LA single-outlier/binomial Filter5x5 kernels plus its NEON strategy, I32 Filter3x3/Filter5x5 AVX2+FMA plus the guarded I32 Filter5x5 AVX/FMA scalar ring, L rank windows through 15x15, and F32 order statistics. A source scan found no active Rayon imports, parallel feature gates, or parallel scheduling under maintained `pool_cpu`; stale comments implying row-parallel execution were removed from serial Filter helpers. Keep Rayon in the later Parallel CPU owner. All work remains unverified; no build, formatter, checkstyle, lint, test, parity, benchmark, or coverage command has run.
- `compute/execution.rs::execute_prepared_shared_with_palette` now obtains Filter3x3's concrete selection from the CPU registry before attempting I32 Arc sharing. The I32 mode owner uses its selected Default scalar proof or AVX2/FMA packed-byte scan, and the non-identity route executes that same prepared plan. The separate SIMD identity helper and backend-based SIMD predicate are no longer used by maintained execution. This remains source-only and unverified; public Filter validation and parameter-order paths have now been source-mapped, while other direct/eager compatibility routes remain pending.

### Core image mode API progress: incomplete and unvalidated

- Materialized pixel access, scalar storage validation, statistics, bbox, and histogram dispatch now consume the exact Mode derived from DynamicImage. Rust putpixel mode APIs take Mode; Python putpixel and raw-data wrappers use mode_identity and enum matches. Scalar color-count helpers also receive Mode instead of I/F spellings. Pillow strings remain at public metadata and error boundaries.
- The same source-only pass converted transparency/band reporting, palette checks, analysis masks, rotate filter decisions, paste mask decisions, and ImageOps fill resolution to exact Mode matches. Color names still cross the existing color parser boundary with the canonical Pillow spelling. These changes preserve each existing mode case, including legacy scalar-fill behavior.
- Encoder rejection checks, quantize source/palette checks, PILfont glyph-mode validation, unpacked byte export, and XBM bitmap validation now also use exact Mode. Explicit raw encoder mode arguments remain Pillow strings because they are codec/packer inputs.
- Encoded header `ImageMode` projection now lives in `image::codec_adapter` and returns the corresponding exact raster `Mode`; 16-bit LA/RGB/RGBA and 32-bit float RGB/RGBA no longer report their byte-mode counterparts before materialization. After decode, the `DynamicImage` tag remains authoritative. The mapping has source review but no build or parity validation.
- `raster::DynamicImage` no longer exposes `image_slash_star::DecodedImage` in its methods. The Pillow-owned `image::codec_adapter` handles decoded pixel-to-raster mapping, raster-to-codec output, and the `ColorType` translation; raster owns its storage-layout enum, distinct from exact `Mode` identity. Raster bounds failures use the Pillow-owned `RasterError`, so raster no longer re-exports codec errors. Pipeline and eager mode-conversion telemetry now compares exact `DynamicImage::mode()` values, and the unused ColorType-to-mode projection has been removed. `PilError` still wraps codec failures at the I/O boundary. Source-only and unvalidated.
- Convert now takes typed source and destination identities in core, and enhancement, paste source preparation, and module Composite no longer recover a destination mode string before conversion. Python and JavaScript parse valid external strings before delegation. Raw-byte codec overrides remain string inputs at their explicit codec boundary.
- ImageOps point validation and module point/eval helpers now use `Image::mode_identity()` as the common resolver instead of duplicating a cached-mode check followed by direct materialization. The resolver uses the planned pipeline identity when available and retains materialization for unresolved transitions. This source-only consolidation has not been compiled or validated.
- Filter mode validation now uses the same exact-mode resolver and known early-rejection names resolve through operation support declarations; a redundant I-mode wrapper branch whose arms pushed the same `PipelineOp` was removed. Source review maps the Python filter-name path, preflight cases, validation-before-`_apply` order, and mode-before-parameter order for kernel and parameterized filter entry points. Color3DLut source and target admission now read the same mode and channel declarations as prepared execution, preserving the prior palette-specific and target-mode error text. These source changes remain uncompiled and parity-unverified.
- Direct scalar point transforms, callable transforms, and PointFloat build their CPU execution context from the image's pinned backend or the active routing policy. The compatibility SIMD request reaches CPU with its centrally detected architecture, and serial-only strategies retain their explicit Default selection. Eager receipts now retain those selections, preserve source-graph receipts, and report GPU-to-CPU fallback; failed execution cancels the partial receipt. This is source-only and still needs frozen parity for mode/error ordering and backend/architecture fallback.
- This is a targeted source migration, not a claim that every image, codec, or operation boundary is typed. Remaining mode-string routes still need inventory, implementation, and error-order review. No build, formatting, checkstyle, lint, test, parity, benchmark, or coverage command has run.

### Resize mode-boundary source progress: incomplete and unvalidated

- Public resize now resolves the exact `Mode` through `Image::mode_identity()` before selecting nearest behavior for `1`/`P`, the `LA`/`RGBA` reducing-gap exception, and the `F` nearest identity shortcut. This uses a planned pipeline identity when available and materializes only when the graph cannot provide one. The chosen identity is passed into resize policy rather than inferred again.
- `apply_reducing_gap` now receives typed `Mode` and checks the four `I;16*` identities explicitly instead of using a Pillow-string prefix. Thumbnail passes its exact mode to its nearest-filter decision, including PA.
- Prepared Resize/ResizeBoxed now dispatch all 25 mode identities to exact typed owners. Public direct Resize queues typed operations, and nested geometry/Thumbnail callers bind prepared children with the same context. `ops/pil_resize` provides shared low-level helpers; generic full-operation wrappers are not active prepared CPU callsites. The prepared Default route performs Pillow's boxed identity/integer-crop shortcuts; architecture guards select that route for those cases, while filtered straight `LA`/`RGBA` crops retain alpha resampling. Float32 bounds are narrowed at validation, guard, and coefficient boundaries; unchanged-axis pass requirements are shared with the native typed-float strategy. Public and direct prepared boxed Resize admission now use the same float32 validator and preserve its baseline validation/error order. Source review confirms the native I;16 owner returns correctly shaped typed buffers across empty dimensions. The extreme-tall rule is restored as vertical-then-horizontal prepared child execution for Pillow modes; `F` retains its specialized mode owner, while CoreOnly typed modes retain their existing behavior. Straight LA/RGBA use explicit premultiplied Mode variants across both passes, with conversion only at the required mode boundary. TODO: finish caller/architecture review for Crop, Reduce, Thumbnail, Pad, and direct Geometry; verify Resize behavior in the frozen CPU parity matrix. Source-only; no build, formatter, checkstyle, lint, unit test, parity, benchmark, or coverage command has run.
- Luma byte convolution has exact SSE2, NEON, and Wasm SIMD128 entry points under `resize/luma8/<arch>/resize_impl.rs`; its checked coefficient guard selects those only for nonempty, unboxed, filtered Luma8 work with native storage and i32-safe taps. The same two-pass fixed-point rounding and scalar tails are retained. Pad's full-width window can execute the selected Luma8 architecture strategy directly. Native-byte vector convolution, bounded i32 fast paths, and coefficient lane plan are under `resize/helpers/native_byte_vector/`; eleven exact byte-mode owners provide SSE2, NEON, and Wasm SIMD128 filtered entry points, with channel and alpha policy selected as compile-time mode parameters. Premultiplied alpha, PA, and raw color-space samples remain native. The declarations use the same checked nonempty, unboxed, i32-safe coefficient guard and retain the same-mode Default fallback. Boxed byte convolution and nearest-vector paths have exact byte mode/architecture owners with float32 bounds and coefficient admission. Exact integer crops and identity boxes use the Default shortcut; filtered straight-alpha crops continue through their selected resampler. One and P remain nearest-only vector strategies. The migrated code contains no Rayon scheduling. This is source-only and unverified.

- Resize/ResizeBoxed and Reduce omitted physical lanes now follow Pillow's
  exact copy/resample rules: Resize identity/exact-crop paths remap source
  lanes; actual resampling clears unused 2-/3-band bytes, while filtered LA
  reconstructs lanes 1 and 2 from final luma after its La round trip. Reduce
  preserves lanes at factor 1 and clears them after an actual reduction.
  Prepared Thumbnail uses these owners; sparse RGB PutPixel/Thumbnail fusion
  also applies Reduce's physical-lane rule. Fused direct Resize-to-Pad owners are L/RGBX (neither has an omitted sidecar); compact modes use prepared Resize followed by Pad lane composition. Every Pad execution route applies lane composition to the resized content before returning the padded result. Source-only; no validation has run. Sources: Pillow 12.2.0
  [`Resample.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Resample.c),
  [`Image.py::resize`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2134-L2237),
  [`Reduce.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Reduce.c),
  and [`Convert.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L103-L134).

- Filter now composes omitted physical lanes after the exact mode/architecture strategy. Filter3x3/5x5 copy the kernel border and zero omitted lanes in the processed interior, matching Pillow's native 2-/3-band output words; BoxBlur/GaussianBlur filter each retained physical byte, and UnsharpMask blends the filtered bytes with the same thresholded formula. Native rank and ModeFilter extensions process omitted bytes independently. No rationale for Filter.c's zeroed padding is documented in the source. Source-only; CPU parity remains pending. Sources: Pillow 12.2.0 [Filter.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Filter.c#L114-L330), [BoxBlur.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/BoxBlur.c#L842-L893), [RankFilter.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/RankFilter.c#L544-L653), and [ModeFilter.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/ModeFilter.c#L364-L464).

- Rotate remaps omitted physical lanes alongside source pixels for identity and exact 90/180/270-degree paths that use the discrete copy strategy. Affine nearest paths sample retained lanes with the same fixed-point geometry and selected Default/vector mapper as visible samples. When source sidecars are absent, LA/La/PA lanes remain implicit because they repeat output luma; RGB-family paths derive the canonical source pad and map out-of-bounds pixels to the configured padding fill. Filtered LA/La/PA output repeats the final luma into omitted lanes, and filtered RGB-family output retains the initialized padding fill. This follows Pillow Geometry.c write behavior and the same prepared architecture boundary as the exact-mode strategy. Source-only; CPU parity remains pending.

### Composite and Paste source-only continuation: incomplete and unvalidated

- The shared support selector now evaluates declarations for the requested `Architecture` before permitted lower-architecture candidates, and both support assessment and preparation use that order. When an operation has only a lower-architecture implementation, selecting it now retains `MissingSpecialization` instead of clearing the reason; Composite is one such declaration set for an AVX2 request when its SSE2 strategy is available. This aligns the inspectable selection with the declared fallback contract. Parity must still verify exact matches, lower-architecture selections, and build/hardware/guard fallback receipts across all operation declarations. Source-only; no runtime validation ran.
- Merge and BlendModule source review (2026-11-01): the typed Merge wrapper retains the baseline shape, band-mode, and size validation order; prepared execution dispatches by exact target Mode. Identity modes retain their native source, multi-band Default owners interleave the declared native samples, and LAB applies the archived A/B bias. The RGB Default owner restores the archived 64-pixel three-L-band loop; exact SSSE3/NEON specializations retain the 16-pixel admission and use the selected guard, with same-architecture wide-shuffle fallback below it. The portable wide routes preserve the archived 8-pixel two-channel and 4-pixel three/four-channel blocks, scalar tails, and empty/sub-block route receipts. BlendModule's typed mode groups and error order match the baseline wrapper; its Default arithmetic preserves f32 fused blending and LAB bias handling, while eligible AVX/FMA and NEON paths retain the finite-alpha, exact-layout, empty-or-at-least-eight-byte guard and 16-byte/padded-tail receipts. Source review only; CPU parity remains pending.
- Composite and Paste mask selectors now match the archived accepted mask identities: `1` and `L` select sample 0; `LA`, `RGBA`, and premultiplied `RGBa` select the alpha byte when requested. The Default Composite byte path retains the archived per-byte `(source * mask + destination * (255 - mask) + 127) / 255` operation and keeps image2 as the output canvas.
- The Composite eager wrapper matches the baseline order: validate image1/mask dimensions, resolve the mask's exact mode, then convert image1 to image2's exact mode if needed. The accepted mask sets are exactly `1`/`L` and `LA`/`RGBA`/`RGBa`; palette metadata for P/PA outputs follows image2. `CompositeNativeByteVector` admission checks exact source/destination identities and byte layouts plus mask identity, layout, and source-size agreement. The prepared architecture plan then routes each byte mode to its corresponding borrowed Paste owner; specialized NEON row owners require the selected Composite L-mask guard and otherwise fall back to the generic same-architecture vector kernel. Wrapper, formulas, tails, and receipts are source-compared; exact-mode owner dispatch, selected-guard consumption, and architecture/build fallback are also traced in the continuation below.
- The CMYK L-mask NEON row now has a source-to-source comparison against archived `native_paste_cmyk_l_masked_row`: both process 16-pixel interleaved C/M/Y/K blocks, use the same widened DIV255 arithmetic and +128 correction, continue with 16-byte then 8-byte samples, and use a padded 8-byte tail for the `true` short-tail policy passed by archived Paste/Composite. Both count each 16/8/padded vector block and count the active padded-tail bytes as scalar-tail work; the selected path receipt is `neon-cmyk-l-blend`. No formula, threshold, tail, or receipt mismatch was found. This is source review only; no kernel was run.
- RGB and HSV L-mask NEON rows now have a source comparison against the archived RGB row: both deinterleave the same 16-pixel/48-byte prefix with `vld3q_u8`, apply one mask sample to each of its three byte lanes, then process complete 16-byte and 8-byte groups and the permitted padded 8-byte tail. The sub-strategy guard keeps the archived 16-pixel minimum; vector-block receipts match the archived row-byte accounting, no scalar-tail receipt is emitted for this L-mask specialization, and the `neon-rgb-l-blend` path is selected only when the prefix actually uses NEON. HSV uses the same byte layout while retaining its exact output identity. No formula, threshold, tail, or receipt mismatch was found. This is source review only; no kernel was run.
- L and LA L-mask NEON rows now have source comparisons against their archived owners. L keeps the 16-pixel threshold, 16-byte NEON groups, exact scalar remainder formula, and matching `neon-blend` receipt counters. LA keeps the eight-pixel threshold, deinterleaves both stored channels with one L sample per pixel, then uses 16/8-byte blocks and a padded 8-byte tail; its active path, vector-block counts, and no-scalar-tail receipt match the archived `allow_short_masked_tail=true` route. Both use the same widened DIV255 rounding as the archive. No formula, threshold, tail, or receipt mismatch was found. This is source review only; no kernel was run.
- RGBA L-mask NEON now has a source comparison against archived `native_paste_rgba_l_masked_row`: both use the exact four-pixel threshold, 16-byte RGBA NEON blocks, then 16/8-byte remainder blocks and an optional padded four-byte pixel tail. The four stored samples, including alpha, use the same per-byte DIV255 arithmetic. Vector-block and active tail-byte counts match the archived padded-tail accounting, and `neon-rgba-l-blend` is recorded only for the selected NEON route. No formula, threshold, tail, or receipt mismatch was found. This is source review only; no kernel was run.
- The archive dispatched PA plus an L mask to a distinct two-byte row kernel. The exact `ImagePaletteAlpha8` Paste and Composite architecture declarations now include that Neon substrategy with a one-pixel clipped-width minimum; the `palette_alpha8/neon/pa_row.rs` owner blends the two stored PA lanes independently, expands each L value across its pixel pair, uses the archived 16/8-byte blocks plus padded 8-byte tail, and records its vector path without scalar-tail work. The generic vector implementation remains the fallback for other masks and source layouts.
- LA/RGBA AlphaComposite support and direct/pipeline callers are source-reviewed against the archived validation order: destination mode, source mode, dimensions, then queue/preparation. The fixed-point serial pixel routine, eight-pixel vector path, zero-alpha preservation, exact-mode output, and RGBA AlphaComposite+Mirror fusion eligibility/receipt flow were also traced to their archived counterparts. This remains source-only and unvalidated.
- Paste's direct public routes are source-reviewed: Python and JavaScript classify host values into neutral input records and delegate to `Image::paste_with_input`; typed Rust `paste` and `paste_at` share `paste_impl`; Draw Bitmap's compact masked route is gated by its own exact destination/mask checks. Source size lookup, placement validation, dimensions-before-mask ordering, source normalization/conversion, and lazy queueing remain in the wrapper. `paste_mask_uses_alpha` now obtains the accepted mask identity and luma/alpha selector from the exact destination mode's `OperationId::Paste` input declaration, removing the duplicate wrapper table.
- Pipeline Paste preparation materializes source before mask, records exact mode/dimensions/layout facts for both, assesses the operation declaration, and consumes the injected selection/context to prepare the destination mode owner. The lazy planner preserves the destination `Mode`; P/PA raw-sample materialization retains palette metadata. The public contract covers normalized same-mode sources, specified compact masked-bitmap sources, exact masks, and unchanged mismatch/rejection errors. Direct and pipeline callers, per-mode SIMD/kernel formulas, block/tail receipts, and selection routing are source-reviewed; runtime pixel and telemetry evidence remains pending parity.
- Composite's generic masked-byte vector route now matches archived source for its exact-mode byte inputs. Both use clipped source rows, the full requested-region mask stride, and the clipped source offset when mapping each stored byte to a mask pixel. Binary `1` masks select source for every nonzero sample; fractional `L` and alpha masks use the same per-byte DIV255 formula, while premultiplied `RGBa` uses the archived `+128` preblend formula. The 16-byte, 8-byte, padded-tail, empty-overlap path, vector-block, and mode-specific tail receipts match: binary tails count the active remainder modulo 16; fractional tails use the archived per-mode exceptions for L/LA/PA/RGB/HSV and the 8-byte remainder for the other modes. The seven specialized L-mask NEON rows were reviewed separately above. This is source evidence only; no route was run. Exact-mode owner selection and architecture/build-unavailable fallback are source-traced; selection receipts and pixel outcomes remain for parity.
- The two unmasked cross-mode RGB routes are now separately source-compared. RGBA→RGB uses the same four-pixel shuffle, padded partial block, ceiling vector-block receipt, no scalar-tail receipt, `vector` path, and empty-region `vector` receipt as archived `simd_paste_rgba_to_rgb`. RGB→RGBA uses complete four-pixel shuffles plus the same per-pixel scalar remainder, floor vector-block count, scalar-tail count, `vector-rgb-to-rgba-copy` path, and empty-region path receipt as archived `simd_put_alpha_rgb_row`/`simd_paste_rgb_to_rgba`. The Default channel mapping also matches the legacy fallback: LA replicates luma and drops alpha for RGB; four-byte sources preserve their first three raw bytes; RGB inserts opaque alpha for RGBA. Masked mapped-source behavior is source-reviewed; architecture guard selection and same-mode fallback are traced below. This is source review only; no runtime validation ran.
- The masked LA/RGBA/RGBa→RGB mapped route also matches the legacy fallback at the output-channel level: the wrapper retains these accepted native source payloads, each RGB result lane samples luma or the corresponding first-three source byte, and the declared mask value blends that lane against the destination. The fallback's intermediate source/destination alpha lane is discarded when it restores the RGB identity, so it cannot affect the three retained channels. Exact wrapper normalization excludes other mapped sources and keeps RGB→RGBA masked input rejected. This source comparison found no output-formula discrepancy. The 25-mode contract and typed owner inventory are source-reviewed below; runtime pixels and receipts remain pending.
- Paste architecture admission/fallback source trace (2026-11-01): `ModeSupport::assess` and `select` share the exact-requested-architecture-first order, lower-architecture permission/build/capability checks, and first-fallback-reason retention. Each Paste mode's `ops.rs` routes the resulting architecture identity to its same-mode owner; `NativeBytes` failure, unavailable architecture/build, or missing specialization leaves the prepared same-mode Default owner selected. The seven exact L-mask NEON subkernels are represented as guards ahead of their generic same-NEON vector entry; each kernel checks the selected guard, and an ineligible specialized row uses the generic vector kernel. `paste_l_mask_neon_guard` and the kernel use the clipped region and mode-specific width minima; `PasteRegion::clipped` matches the archived CPU and SIMD region calculation. The RGBA→RGB and RGB→RGBA inner routes are source-layout-specific helpers within their declared exact mode/architecture owner. Source trace only; parity must still establish selection, fallback receipts, and output pixels on each available architecture.
- CompositeModule, Merge, and BlendModule source review is closed; runtime behavior remains unverified. Paste's 25 matching support classifications, prepared variants, exact typed preparation routes, mode owners, architecture admission/fallback, and formula/tail comparisons are source-reviewed. The Python wrapper order review reopened on 2026-11-02: staged source/destination/mask loading now preserves palette boundaries and box-as-mask uses the loaded wrapper image; input coercion and custom coordinate timing remain TODOs in CPU_REFACTOR_PLAN.md and the Paste wrapper notes below. Actual pixels and telemetry receipts remain pending parity. No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage command has run.
- GetChannel mode-contract source review (2026-11-01): Pillow [`ImagingGetBand`](https://github.com/python-pillow/Pillow/blob/main/src/libImaging/Bands.c) rejects non-`UINT8` storage before selector validation, then copies a one-band byte image without changing its mode. The maintained wrapper now declares `1` and `P` as mode-preserving identities (including P palette retention); it rejects I/F before materialization and index checks; native I;16* identity reads are CoreOnly with `NO_PILLOW_SUPPORT` and preserve the exact typed mode/sample buffer. Internal u16/f32 multi-band outputs remain typed. Other supported multi-band routes use their prepared `ExtractBand` child. The exact selector/output contract is source-reviewed, but direct identity/no-op receipt behavior, L shared-buffer, tiny-RGB eager, deferred execution, architecture fallback, and telemetry paths still need the frozen 25-mode CPU parity matrix. No validation ran.
- GetData caller/context source review (2026-11-01): allocated byte-band reads select the exact-mode contract, use a caller-owned buffer, and route declared SIMD/GPU terminal projections through prepared `ExtractBand`; unsupported projections fall back to the per-mode CPU Default owner. Direct palette-index views use the `Palette8` declaration, and formatted I/F/I;16 reads use typed scalar owners. Full-buffer, scalar-only band, and formatted I/F/I;16 reads now inject the image-pinned/active backend policy into exact-mode CPU strategy selection and record selected/fallback receipts for already materialized host reads. Lazy terminal receipt composition remains pending. GPU terminal eligibility also remains outside the combined support declaration. These routes now have source TODOs and remain in the frozen per-mode CPU parity/receipt matrix; no validation ran.
- CoreOnly high-depth statistics/GetColors source review (2026-11-01): Histogram and Entropy now scan native u16/f32 samples into the established 256-bin layout without a converted image; Entropy composes the prepared Histogram child under one CPU context. GetColors now declares all 25 modes at the operation level, rejects Pillow's I;16* modes before materialization, counts every byte mode from its exact raster carrier, and counts internal u16/f32 pixels by native sample keys with typed output tuples. Pillow byte-color hash-slot ordering and I/F scalar hash behavior remain in their shared specialized helpers. Stat reduces the prepared 256-bin Histogram output for byte, I;16, and CoreOnly modes without creating a converted source raster; I/F retain their exact scalar formula in their mode owners. The masked Stat wrapper now prepares its Histogram child under the selected Stat context, routes the bins through the selected exact-mode Stat owner, and reduces native u32 bins without a widened f64 histogram buffer; it records Stat selection for materialized source/mask inputs. CPU mode × Architecture parity and lazy terminal receipts for Histogram, Entropy, GetColors, and Stat remain TODO; no validation has run.
- CoreOnly histogram bin contract (2026-11-01): LA16/RGB16/RGBA16 and RGB32F/RGBA32F retain the 256-bin-per-channel result shape. u16 binning uses `(sample + 128) / 257`; f32 clamps to `[0, 1]` and rounds the scaled value to `[0, 255]`. These formulas match the raster's established `FromPrimitive` byte scaling while calculating indices directly from native typed samples; there is no cross-mode cast, converted image, or intermediate sample buffer for reuse. Masked and unmasked scans are separate, keeping the unmasked loop free of a per-pixel mask check. The user confirmed this direct native scan as the chosen performance path; no conversion or staging raster is to be introduced for code reuse. Pillow-mode parity, CoreOnly edge-case coverage for u16/f32 boundaries and masks, and benchmark evidence remain pending.
- Lazy output identity review (2026-11-01): supported routes obtain planned output modes from the operation's `ModeSupport::output` declaration. Convert, ConvertMatrix, Merge, RemapPalette, Color3DLut, ConvertLab, Grayscale, Colorize, and EffectNoise declare their rejected-route metadata output through `OperationSupport::rejected_output`; the planner resolves that rule against any typed target parameter without a second operation-to-mode switch. The source audit found no additional rejected source-mode route that exposes a changed raster result before its error: mode-preserving routes retain their source identity; rejected Equalize has no raster result; PutAlpha wrappers reject unsupported source modes before queueing; raw invalid `push_op` descriptors retain their deferred operation error instead of claiming a result mode. Mismatched PutData payload descriptors bypass the mode-preserving identity fast path, and cached-prefix reuse requires the declared output to equal the exact source identity. These metadata and error outcomes remain pending frozen CPU parity. Source-only; no validation ran.
- Indexed enhancement caller source audit (2026-11-01): Brightness and Sharpness validate exact-mode support before queueing, so P/PA follow their declared wrapper rejections. Contrast and ColorSaturation use the eager base-conversion/Blend path for modes without a prepared strategy; their `OperationModeRoute` declarations expose the intermediate mode and child rejection stage. Direct indexed `push_op` admission checks exact P/PA support before palette expansion, and the planner leaves a mode-preserving palette-unsafe output unresolved for the existing materialization path. Pending: verify indexed metadata, eager base outputs, and public wrapper error order in the frozen CPU parity cases. No validation ran.
- Analysis support placement (2026-11-01): `analysis/ops.rs` now owns the exhaustive mode classifiers and descriptors for Histogram, Entropy, GetColors, Stat, and GetExtremaBytes beside their operation family entries. The execution helpers consume these declarations for admission and preparation; they no longer own or re-export the support contracts. The `ProjectionOutput` receipt type lives with projection helpers so `analysis/ops.rs` remains under the 1,000-line cap. This structural move is not compiled or runtime-validated.
- Quantize source migration (2026-11-01): The public Image wrapper routes direct Quantize through the CPU operation owner with injected CPU/Architecture context, palette metadata, exact source mode, P output, and declared parameter guards. L reads native luma samples; P resolves its borrowed RGB palette once into a fixed lookup table and reads native indices; both use statically dispatched quantizer sources, never a relabeled/intermediate DynamicImage. RGB8 and RGBA8 retain their exact-mode Default owners. All 21 CoreOnly identities now dispatch through their exact mode `ops.rs` and Default quantize implementation under `imageops/<mode>/default`; the implementations read native carriers without a converted DynamicImage/RGB/RGBA raster. RGBa and RGBX preserve the prior alpha-aware octree route; I;16 retains clipped byte semantics, while other u16/f32 carriers apply the raster FromPrimitive mapping during sample reads. Each extension owner has a nearby `NO_PILLOW_SUPPORT` comment. The operation contract explicitly restricts methods for RGBA/RGBa/RGBX; pre-materialization wrapper validation remains limited to Pillow RGBA to preserve palette/error ordering, while the prepared contract enforces the same restriction for RGBa/RGBX. Their CoreOnly strategies retain the alpha-aware octree and alpha palette. MaxCoverage and median-cut k-means now access native sources without staging converted pixel lists. Custom-palette mapping reads native L or RGB buffers directly. Quantization helpers remain shared with color conversion only where their sample contract is identical. Materialized Quantize calls now record the requested Backend and exact-mode `StrategySelection`; lazy-source receipt composition remains a TODO. Mode × Architecture parity and CoreOnly extension evidence remain pending; no validation has run.
- Draw Bitmap mask source correction (2026-11-01): Pillow [`Paste.c::ImagingFill2`](https://github.com/python-pillow/Pillow/blob/main/src/libImaging/Paste.c) accepts `1`, `L`, `RGBA`, and `RGBa` masks. The maintained `BitmapMask` now reads the alpha byte from both RGBA identities; RGBa applies the archived PREBLEND arithmetic, including at zero mask alpha. For nonzero L coverage on alpha-bearing destinations, Pillow's integer expression promotes non-alpha coverage only when destination alpha is exactly zero; the alpha channel always uses the original coverage. The byte-mode LA/La, PA, RGBA/RGBa, RGB/CMYK, and raw four-byte owners now follow these branches; PA writes its palette index and alpha without treating the index as luma. Source-derived code only; no build or runtime verification ran. Pending: I;16* byte-order parity, direct Text ink/palette/error ordering, public Bitmap fast-route and metadata reconciliation, and CPU parity/receipts.
- CPU byte PREBLEND placement (2026-11-01): moved the scalar Pillow PREBLEND byte formula to `pool_cpu/helpers/bytes/preblend.rs`, the narrow backend helper shared by Draw Bitmap and Paste's scalar byte/I;16 owners. Their source mode and carrier selection remain with each operation; only identical arithmetic is shared. No validation ran.
- Draw Text mask layout (2026-11-01): mode dispatch now specializes coverage-only and RGBA FreeType masks before entering each mode's Text strategy. Mode owners read the original borrowed mask bytes directly; the former coverage-to-RGBA expansion and its per-call allocation are removed. RGB-family owners still read embedded mask RGB when the declared mask is RGBA. This preserves the current mode algorithms at source level; ink/palette/error-order parity and the remaining Draw TODOs are pending. No validation ran.
- I;16* Draw Text native path (2026-11-01): Pillow's `ImageDraw` chooses an `L` font mask for I;16* and `Paste.c::fill_mask_L` blends the two ink bytes at the native two-byte sample stride. `fill_mask_RGBA` steps through the raw row byte prefix at one byte per mask pixel and uses alpha coverage. The exact-mode owner now clips the glyph rectangle and applies each mask layout directly to the u16 carrier's byte view; it removes the previous full-canvas u16→u8 projection and rescaling pass. This avoids converting the mode or allocating a staging raster. Each declared I;16* byte-order identity remains pending parity; no validation ran.
- P-mode Draw Text clipping (2026-11-01): the palette-present and palette-absent branches had different source-mask loop bounds despite `ImagingFill2` using the same destination clip and mask offset for P. Replaced both with one native palette-index loop over the clipped rectangle; the wrapper still requires prepared palette knowledge so metadata handling and error order remain explicit. Negative-origin source clipping now follows Pillow's mask offset. CPU parity and metadata/error-order checks remain pending; no validation ran.
- Draw Bitmap source borrowing (2026-11-01): after mask mode and dimensions are admitted, the generic direct Bitmap route now borrows bytes from the materialized native mask instead of building a second full-size `Vec<u8>` via `getdata(None)`. The direct typed mode kernel continues to own destination writes. Error ordering, lazy-mask materialization behavior, and backend route parity remain pending; no validation ran.
- I;16* Draw Bitmap source correction (2026-11-01): Pillow's `Paste.c::fill_mask_1`, `fill_mask_L`, `fill_mask_RGBA`, and `fill_mask_RGBa` operate on the I;16 raster's byte view. `L` blends both ink bytes at the 2-byte pixel stride; `1`, `RGBA`, and `RGBa` step through the raw row byte prefix at one byte per mask pixel, with `RGBa` using alpha and PREBLEND. The maintained owner now clips the mask region and applies those byte operations directly to the native u16 carrier, removing the prior u16→u8 luma projection and full-frame rewrite. This source-derived behavior remains unverified for each I;16* mode identity; no checks or parity ran.
- Draw `fill_mask_L` integer rule correction (2026-11-01): the C expression uses integer division, so only a zero destination alpha promotes non-alpha coverage to 255; alpha values 1–255 retain original coverage. Updated the shared Bitmap helper and PA Text path to reflect that exact branch. This is source review only; no checks or parity ran.
- F32 rank selection correction (2026-11-03): source comparison found that the archived Rust implementation's partial-comparison sort is not Pillow 12.2's `RankFLOAT32`. Pillow uses the same in-place Wirth partition for UINT8, INT32, and FLOAT32; its `<` comparisons make NaNs and signed-zero ties depend on partition order. The maintained Default owner now mirrors that partition in safe Rust for all F32 windows. The vector strategy remains admitted only for nonempty exact F32 buffers, odd windows through 9x9, valid ranks, and samples with no NaNs or mixed zero signs. Pillow source: [`RankFilter.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/RankFilter.c). This source correction has no runtime verification; freeze NaN payload, mixed signed-zero, compatible-tie, operation, architecture, and fallback-receipt parity cases before claiming parity (`pillow-rs#brg0`).
- Rank-filter boundary source review (2026-11-03): Pillow 12.2's Python `RankFilter.filter` expands the image by `size // 2` on every side, and `ImagingExpand` repeats the nearest edge samples; `ImagingRankFilter` then shrinks that padded image by the same margin. The maintained same-size clamped gathers are equivalent for nonempty inputs and avoid allocating the temporary expanded raster. For zero-width or zero-height input and rank windows of size 3 or larger, Pillow 12.2's positive-margin expansion dereferences edge storage; the Rust path safely returns an empty same-mode result. Keep those empty-image cases in the frozen parity matrix as a deliberate safe divergence; size 1 has no expansion margin and is not part of this divergence. Sources: [`ImageFilter.py`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageFilter.py#L1537-L1547), [`Filter.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Filter.c#L1242-L1333), [`RankFilter.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/RankFilter.c#L581-L620). This source review does not replace runtime parity.
- PutAlpha support and wrapper trace (2026-11-01): the public wrapper rejects One, La, LAB, YCbCr, and HSV, and reports the Pillow conversion errors for One/YCbCr/HSV. The operation support declaration now matches those rejections, marks the existing native u16/f32 carrier outputs CoreOnly, and the lazy output planner no longer predicts a result mode for rejected YCbCr/HSV descriptors. This is admission metadata only; the kernels were not changed. `putalpha_data` validates destination-mode support before materializing the mask, matching Pillow's first validation step. Pillow 12.2 then promotes a supported destination before validating an image mask; the Rust boundary does not queue that transition until every mask check passes. Mutation-on-mask-error and Python metadata behavior remain pending review against the pinned source. No validation ran.
- RGBa PutAlpha source correction (2026-11-01): Pillow's [`Image.putalpha`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py) converts RGBa to RGBA before replacing the alpha band; [`Convert.c::rgba2rgbA`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c) copies RGB at alpha 0/255 and otherwise computes clipped integer `(255 * channel) / alpha`. The archive's generic carrier conversion retained premultiplied RGB, so constant and mask PutAlpha now fuse Pillow's unpremultiplication into the one required native RGBA output allocation, without retagging samples or staging another raster. The scalar unpremultiplication helper is shared under `pool_cpu/helpers/bytes` with Convert. Source review only; CPU parity remains TODO for old-alpha values 0, 1, 254, and 255 under both inputs.
- Maintained CPU projection scan (2026-11-01): searched active `pool_cpu` outside `color/convert` for calls to `to_rgba8`, `to_rgb8`, `to_luma8`, `to_generic`, `into_rgba8`, `to_luma_alpha8`, and typed luma/RGB output methods. The only call found is P Equalize's palette expansion to RGB, which is part of Pillow's operation semantics and is documented at its mode owner. This is source inspection only; no validation ran.
- DynamicImage mode-assignment audit (2026-11-01): `DynamicImage::with_mode` preserves bytes and explicitly assigns another identity only when the carrier family matches; callers must certify sample meaning, and this helper is not a conversion strategy. Source review covered decode/frombytes and generated-image adapters; GPU result metadata; Convert, Enhance, Paste, ParamFilter, and Eval result carriers; and the test-only resize compatibility helper. Adapter and operation callers pair their explicit requested `Mode` with a carrier produced for that mode, while internal high-depth identities return unchanged or reject incompatible storage. The test-only resize helper now fails immediately if an output carrier cannot preserve the source identity instead of silently returning the default carrier tag. The uncached-only pipeline retag has been removed. Cached and uncached evaluation now compare the producer's exact variant with declared output metadata and return an internal error on mismatch rather than silently reinterpreting same-carrier samples. Remaining: audit declared and lazily planned outputs against every producer strategy and reconcile any mismatch at its owner. No validation ran.
- Resize compatibility reachability (2026-11-01): source search found no production call sites for the generic `ops/pil_resize::{pil_resize,pil_resize_boxed}` wrappers or `pil_preserve_mode`; exact CPU Resize owners use prepared typed strategies and shared low-level coefficient/sample helpers. The generic wrappers and their mode-inference adapter are now compiled only for existing local unit tests, leaving the production helper facade focused on reusable primitives. `X86FmaToken::detect` is test-only; production F32/I32 AVX/FMA owners construct the token from injected `ExecutionContext` and selected `Architecture`. This is a source-call-graph cleanup only; no validation ran.
- Convert nonstandard-source routing review (2026-11-01): `Image::convert` checks the exact source-mode Convert declaration before its fallback routes. CMYK, YCbCr, HSV, P, PA, I, and F each declare L and LA, and their Default Convert owners produce those destinations directly; removed the unreachable generic grayscale/luma and P-to-RGBA branches with the unused YCbCr luma helper. Every candidate pair in `try_convert_between_nonstandard_modes` is covered by those source-mode declarations, so the unused wrapper path is removed. Reviewed the remaining `try_convert_nonstandard_source` target classes: binary and palette routes precede the support check, I;16 and LAB have dedicated composed routes, La/RGBa and RGBX have explicit wrapper routes or rejections, and the remaining accepted targets are declared for each nonstandard source. Removed the generic expansion fallback as unreachable. The separate eager premultiplied-alpha and RGBX compositions still need archived error-order, output-identity, and injected CPU receipt/context review at their owners. This is source-call-graph review only; no build, lint, test, parity, or benchmark ran.
- GetChannel/Split support-route review (2026-11-01): exact mode contracts expose GetChannel selector counts, P/1/I;16 identity outputs, I/F rejection, CoreOnly u16/f32 bands, and the ExtractBand child declaration. `OperationSupport::routed` now reports each exact mode's wrapper path: direct identity or palette copy, guarded loaded-L shared-buffer identity with a child fallback, guarded tiny-RGB eager child with a lazy child fallback, and per-band Split children. `OperationModeRoute` exposes the declared pipeline child IDs, and GetChannel/Split map those IDs to the queued `PipelineOp` variants instead of independently naming their children. Deferred terminal receipts, metadata/error order, and mode × architecture parity remain pending; no validation ran.
- GetChannel/Split route follow-up (2026-11-01): `OperationSupport::routed` now reports the per-mode wrapper-owned identity/palette copies, loaded-L shared-buffer identity/child fallback, tiny-RGB eager child/lazy child routes, and Split per-band child routes. The loaded-L shared-buffer path now reports `native-identity`; no architecture is selected for this no-kernel result. Direct `1`/P/I;16 and Split identity-copy receipts remain pending with deferred terminal receipts, metadata/error order, fallback checks, and mode × architecture parity. Source-only; no validation ran.
- GetChannel/Split route declaration update (2026-11-01): `OperationModeRoute::WrapperOwned` and `OperationRoutePath` now describe the per-mode wrapper paths at each operation entry: identity and palette-preserving copies; loaded-L exact-sized shared-buffer identity on the CPU-only active route followed by ExtractBand fallback; tiny-RGB eager CPU extraction followed by the lazy child; and one ExtractBand child per logical Split band. The mode output contracts and existing child declarations are unchanged. Direct and deferred receipts, metadata/error ordering, architecture fallback evidence, and CPU parity remain TODO; this source-only route-model update is unvalidated.
- PutAlpha CoreOnly wrapper alignment (2026-11-01): `validate_putalpha_mode` now delegates to the matching `PutAlpha` or `PutAlphaData` support declaration, allowing the exact native u16/f32 strategies for I;16 identities and internal high-depth modes while removing the duplicate wrapper mode list. The per-mode support marks these routes CoreOnly and the pipeline planner declares their typed output identities; `One`, I, F, YCbCr, HSV, La, and LAB retain their prior Pillow errors. This exposes the implemented Rust extension without changing its scan or alpha algorithms. `NO_PILLOW_SUPPORT` is documented at the wrapper and mode owners. Source-only, unvalidated.
- Convert eager premultiplied-alpha/RGBX source review (2026-11-01): compared the remaining wrapper compositions with the pre-refactor `3f9dfd403` dispatch. Exact source/target strategies are checked first. The fallback premultiplied route retains the archived P rejection, L-like/I;16 errors, and RGB-normalized composition; the RGBX route retains its LA/PA-to-RGBA branch, palette-alpha correction for P, RGB normalization for other modes, and exact RGBX result identity through the compatible four-byte carrier. No conversion formulas changed. TODO `pillow-rs#3ptb`: make these source/target wrapper paths visible beside Convert's operation support and carry one injected Backend/Architecture context through nested eager conversion and receipts. Runtime output/error parity and architecture-selection evidence remain pending; no validation ran.
- Analysis native-reader source audit (2026-11-01): searched active `pool_cpu/ops/analysis`, `ops/analysis`, and Image statistics/entropy callers for raster conversion APIs. Analysis scans and reducers read exact-mode native buffers; high-depth histogram binning remains direct u16/f32, and public byte-valued readers use only their declared per-sample result projection. No converted staging raster remains in these paths. Direct/deferred receipts and mode × architecture parity remain pending; no validation ran.
- I;16 Histogram scan path (2026-11-01): retained Pillow's archived first-`width`-bytes-per-row behavior while specializing endian choice at compile time for native, I;16L, and I;16B owners. Masked and unmasked loops are separate, and byte extraction reads the native `u16` lane directly without constructing a per-sample byte array. This changes loop structure only; exact endian, odd-width, and masked-prefix parity remains TODO. No validation ran.
- Analysis mask/error-order audit (2026-11-01): compared active Histogram, Entropy, masked Stat, and GetColors wrappers with `3f9dfd403`. Histogram/Entropy and masked Stat materialize source before mask, then materialize mask, check dimensions before accepted mask mode (`1`/`L`), and only then prepare the source operation. The typed `ImageAnalysisMask::Invalid` branches return AttributeError before materialization. Masked scalar I/F Histogram rejection occurs after mask validation, as before. GetColors rejects I;16 identities before materialization, and negative `maxcolors` still validates mode then materializes before returning `None`. No source-order discrepancy found in image-mask routes. This remains source-only; no validation ran.
- CPU-only eager Filter route alignment (2026-11-01): UnsharpMask and ModeFilter now share `Image::cpu_arch_execution_route` to capture the requested backend and injected CPU Architecture in one routing snapshot. When an unpinned CPU-only filter sees GPU preferred and the SIMD compatibility alias active, it records the GPU fallback and can still select the declared CPU architecture. Explicit GPU pins retain CPU Default fallback. Filter arithmetic and lazy pipeline routes are unchanged; this source change is unbuilt and unvalidated.
- CPU-owned direct route alignment (2026-11-01): Histogram, Entropy, masked/direct Stat, GetColors, GetBBox, Quantize, eager UnsharpMask/ModeFilter, Draw, GetProjection, and GetData now obtain the routed CPU Architecture through `Image::cpu_arch_execution_route` or its policy-aware variant. Scalar point/eval operations use the same strict route resolver; advisory callers retain their previous backend-state error boundary. An active SIMD Architecture survives CPU fallback from an unpinned GPU preference, while an explicit GPU pin stays CPU Default. Eager telemetry now distinguishes the SIMD compatibility request from a true host fallback: prepared selection records architecture fallback, and GPU-to-CPU remains a backend fallback. No pixel formulas or algorithms changed. Remaining direct/eager caller routing and receipts are pending review; source-only and unvalidated.
- Convert eager context injection (2026-11-01): binary, matrix, I;16-target, and palette-wrapper CPU conversions pass the original Backend request plus the already-resolved CPU Architecture context into eager child strategy execution. GPU preference can fall back through active SIMD, explicit GPU pins remain CPU Default, and wrapper-owned palette/GPU fallback reasons are retained. A lazy source is a separate Image graph: its materialization independently runs `prepare_execution` with that graph's pinned/active Backend route before the wrapper's child executes. Do not inject the child context into source materialization. Remaining TODO `pillow-rs#3ptb`: reconcile nested composition receipts. All arithmetic/output logic is unchanged; source-only and unvalidated.
- Convert wrapper-composition support view (2026-11-01): `OperationSupport` now enumerates exact exceptional source/target routes with ordered child transitions, scope, output identity, and wrapper rejection stage. The public Convert wrapper dispatches LAB, premultiplied, and RGBX compositions from that same descriptor; direct one-step target strategies remain in exact-mode support. The shared descriptor types live in `compute/registry/support/target.rs`, and Convert's operation-specific classifier lives in `color/convert/helpers/target_compositions.rs`. Pending: reconcile nested composition receipts, complete the archived SIMD source/target/guard map, and review remaining direct callers and parity. No validation commands ran.
- Callback write route capture (2026-11-01): the reusable `PutDataExecutionContext` now retains both the selected Backend request and its CPU Architecture context for the entire callback-driven sequence, so a callback changing global backend preferences cannot split one write operation across route policies. Synchronous I;16 byte writes and scalar PutPixel use one advisory route snapshot for both execution selection and receipts. Existing native writers and callback mutation order are unchanged; source-only and unvalidated.
- GetProjection route correction (2026-11-01): direct eager and already-materialized pipeline calls now obtain selected Backend, strict-request status, and CPU Architecture from one route snapshot. The RGB GPU path remains intact; unsupported GPU/mode routes use the selected CPU Architecture, including active SIMD when GPU is preferred, and record the actual CPU architecture in the fallback reason. Existing exact-mode projection kernels are unchanged. Runtime selection/fallback receipts and mode × architecture parity remain pending; source-only and unvalidated.
- ImageDraw CMYK Text comparison for `pillow-rs#brg0` (2026-11-01): compared baseline `3f9dfd403` `draw/text/composition.rs::text_compose_mode_cmyk` against `pool_cpu/ops/draw/cmyk8/default/text_impl.rs`. The archived `to_rgba8()` path copied the four-byte RgbaImage carrier and restored CMYK metadata; the mode-local owner now writes that same carrier directly. Integer-fill ink mapping, RGBA-mask alpha versus grayscale coverage, signed clipping, per-channel `pil_blend`, and exact CMYK output agree by source inspection. This does not establish the historical parity case; focused runtime reproduction remains gated until CPU source completion. No tests or parity ran.
- Rotate I;16* architecture admission source comparison (2026-11-01): baseline CPU `rotate_arbitrary_luma16` samples nearest coordinates from the floating Geometry.c affine, while the archived raw-byte SIMD path rounds affine coefficients to signed 16.16 and advances integer coordinates. The old two-byte path also requires exact I;16 endian-specific fill/output mapping. The four exact I;16 owners remain Default-only so SIMD requests fall back to the native-u16 implementation; do not add the archived byte kernel until it preserves that contract. Boundary parity and selection receipts remain pending; no validation ran.
- ImageDraw RGBA Text source comparison for `pillow-rs#brg0` (2026-11-01): baseline `draw/text/composition.rs::text_compose_rgba` and the exact-mode RGBA Default owner use the same coverage-weighted alpha blend, fill-only RGB branch for ordinary masks, embedded-color RGB blend, and stroke-then-fill order. The new owner reads coverage/ink from the original mask layout instead of allocating an expanded RGBA mask; output identity and metadata reset match by inspection. No source discrepancy found. Historical bytes-mismatch cases still require focused runtime reproduction after CPU source completion; no tests or parity ran.
- Convert support declaration follow-up (2026-11-01): early `La` source/target validation now reads the same `OperationTargetComposition` rejection used by the operation-level Convert support view, while retaining the matrix exception and pre-target-validation error order. Pillow target validity derives from `Mode::pillow_name()` so the wrapper no longer owns a duplicate spelling list. Nested eager Convert telemetry remains pending at the shared eager CPU executor: it currently resets the source graph's completed receipt before recording the child selection. No build, formatter, lint, test, parity, or benchmark commands ran.
- Convert support lookup fast-path review (2026-11-01): source/target composition classification now checks the exceptional LAB, premultiplied, RGBX, and `La`-guard families before looking up source-mode support. Ordinary Convert target queries therefore remain on the direct source contract without classifying a mode composition. Source-only; no verification commands ran.
- CPU ownership boundary scan (2026-11-01): maintained `pool_cpu` outside `pool_cpu/legacy` contains no Rayon or parallel-iterator references, and the source module graph contains no active `pool_cpu::legacy` import or legacy path attribute. This confirms the archive is not compiled into maintained CPU execution and parallel scheduling remains outside the CPU pool. Text/source scan only; the archive accounting, dead-code review, size limits, and all runtime validation remain pending.
- Archived Convert SIMD route comparison (2026-11-01): the legacy SIMD preflight admits exact L/LA/RGB/RGBA byte carriers and explicitly named aliases; RGBa alpha transforms, CMYK→RGB, I;16 source conversion, and One/L→I;16 use separate routes. The maintained One architecture guards still enumerate exactly the four I;16 targets. The full exact-mode source/target/architecture matrix, alpha-only output lanes, build availability, and receipts remain source-audit TODOs. No validation commands ran.
- `pillow-rs#brg0` Draw source comparison (2026-11-01): `draw/text.rs` has no diff from `3f9dfd403` in its multiline layout, line spacing, alignment/anchor positioning, or optional-font-size wrappers; the changed inner path now selects the exact-mode Draw strategy. This accounts for centered-anchor, three-line-spacing, and explicit-font-size routes at source level. CMYK Text's direct carrier loop matches the archived four-byte copy/restore route, and RGBA Text keeps the archived coverage/color-mask and stroke-before-fill ordering. Historical exact-output cases still need focused parity after the CPU source gate; no tests or parity ran.
- Draw P palette input resolution (2026-11-01): Pillow 12.2 `ImageDraw._getink` resolves string/tuple colors through the image palette before native Draw dispatch. Python and JavaScript now use the core stateful Draw resolver, which delegates 3/4-component P colors to the shared palette allocator and passes the returned index to mode kernels. Palette updates replace indexed wrapper metadata directly; for Bytes and pending pipelines an empty metadata layer preserves source operations and avoids decoding pixels just to append a color. Empty-operation overlays pass through the exact DynamicImage variant. Integer indexes and P images without an attached palette retain the existing path. TODO(pillow-rs#3ptb) records malformed short component tuples and full-palette background/pending-transparency reservations. Source-only; no build or runtime verification ran.
- Draw Bitmap fast-route source trace (2026-11-01): exact-mode CPU Bitmap selects Default because the request is not a geometry-span batch. Its RGB specialization delegates to `rgb8/default/bitmap_impl.rs`, whose native-mask view reads RGBa alpha and applies `PREBLEND`; the general raw-byte owner uses the same `BitmapMask` contract. The non-CPU public Paste shortcut is limited to 1/L masks and the explicit destination/context combinations in `bitmap_masked_paste_matches`; its equivalence with every mode owner's ink, palette, metadata, and deferred receipt remains a TODO. No validation ran.
- CoreOnly Histogram design decision (2026-11-02): user selected direct native u16/f32 scans into the existing 256-bin-per-band result for CoreOnly multiband modes. Optimize the typed scan while preserving the chosen binning contract; do not convert sample types or stage an intermediate raster to reuse byte-mode code. Edge, mask, and performance evidence remain pending; no checks ran.
- Fused GetProjection terminal selection (2026-11-01): RGB/CMYK PutPixel terminals retain one prepared `StrategySelection` through CPU terminal execution and telemetry, preserving selected architecture and strategy fallback reason. CPU Default versus architecture execution is chosen from that selection, so a `Backend::Simd` compatibility request that falls back to Default uses the serial kernel. RGB/CMYK kernels and the CMYK positive-write optimization consume that same selection without reselecting during execution. Source review only; lazy receipts for other analysis terminals and CPU parity remain pending. No validation ran.
- Analysis family source-size correction (2026-11-01): moved GetData caller-owned band-count/support queries and terminal-route helper into the existing GetData execution module, re-exporting them from `analysis/ops.rs`; operation support descriptors and strategy selectors remain at the family boundary. `analysis/ops.rs` is now 967 physical lines. No behavior change intended; source-only and unvalidated.
- PutAlpha Python metadata-on-error follow-up (2026-11-01): the Rust wrapper preserves Pillow's destination mode promotion when later alpha-mask or input validation fails. The Python facade now refreshes an already-observed `Image.info` mapping in `finally`, so the retained mapping sees any core mutation on both success and failure. No mode or pixel algorithm changed. Source-only; CPU parity remains pending.
- Draw P palette facade synchronization (2026-11-01): mutating Python Draw calls now synchronize the Rust-owned image in `finally`, so a palette color allocated before a later drawing error remains visible on the original image. `_sync` also refreshes an already-cached `ImagePalette` object after Draw updates the core palette. Remaining TODOs cover Pillow's short tuple behavior and reserved background/transparency slots on full palettes. Source-only; no validation ran.
- Draw P palette reservation follow-up (2026-11-01): The core Draw context initializes reservations from native compatibility metadata, and Python Draw refreshes them from the current `Image.info` background/transparency values before color resolution. The existing full-palette allocator skips those indices and keeps its reverse scan for unused entries; no image-level staging was added. The remaining source TODO is malformed short-tuple behavior. Source-only; no validation ran.
- Typed ImageColor descriptor (2026-11-01): `ImageColor.getcolor` now receives `ColorOutputMode`; Python and JavaScript binding entry points parse the public Pillow `ImageMode` string before calling the typed core function. The descriptor preserves accepted names such as `I;32` and `I;16S` without treating them as `raster::Mode` identities, and parser failures retain Pillow's `KeyError`. Existing Rust callers now use enum variants. Source-only; no build, unit test, parity, or lint ran.
- GetChannel/Split materialized identity receipts (2026-11-01): concrete `Loaded` and `Paletted` GetChannel identity routes validate the selector from the exact mode's band contract before returning a copy. `Loaded` shares its pixel Arc and reports `native-identity`; `Paletted` avoids a redundant materialized-index clone but its public `Image::copy` still clones index bytes, so it reports `native-copy` and accounts the output host buffer. `Bytes`/`Pipeline` caches successful at call entry use `native-identity`; sources first materialized during the call retain materialization-before-index-check order and defer combined terminal receipt composition, avoiding replacement of graph/decode telemetry. Single-band `Split` reports the shared identity path except for cached `Bytes` P and direct `Paletted` P inputs whose palette-index samples are cloned and measured as `native-copy`; metadata clearing remains unchanged. Source-only; no checks or parity ran.
- GetChannel eager-route guard declaration (2026-11-01): `OperationRoutePath::EagerCpuChild` now carries typed wrapper guards. The RGB `GetChannel` descriptor lists the concrete Loaded source requirement, requested CPU/SIMD backend set, and inclusive 256-pixel cap beside its prepared `ExtractBand` child; the fallback child remains ordered afterward. The wrapper obtains its full `OperationModeRoute` once and evaluates this exact guard array before choosing eager execution, so the source no longer has a separate mode/size/backend predicate. `OperationRouteGuard` is re-exported through the support facade and crate root. Source-only; no checks ran.
- GetChannel shared-buffer guard declaration (2026-11-01): The loaded-L identity path now carries its Loaded-source, requested-CPU, and exact one-byte-per-pixel guards in the mode's `OperationRoutePath`; the wrapper consumes that same descriptor before returning a shared-buffer identity result. Backend request resolution is factored from CPU Architecture context creation, so this no-kernel path does not select an Architecture. If guards fail, normal child routing remains available. Source review only; deferred materialization receipts, metadata/error ordering, fallback evidence, and mode × architecture parity remain TODO; no validation ran.
- PutPixel→Thumbnail fusion admission (2026-11-01): RGB fusion preflight now resolves requested Backend, strict-request status, and CPU Architecture through one Image route snapshot, then passes its Architecture context to the CPU support guard. It preserves the existing admission contract: pinned CPU/SIMD requests or a sole active CPU/SIMD backend may fuse; GPU, multiple active preferences, and backend-state errors fall through. No kernel arithmetic or output behavior changed. Source-only; no build, formatting, lint, tests, parity, or benchmark ran.
- GetData single-band view context (2026-11-01): the direct P/1/L byte view now passes the Backend/Architecture context resolved by its owning Image into the GetData strategy selector after confirming that the route is CPU-compatible. It no longer reconstructs a context from the CPU label. This path remains CPU-only and has no architecture kernel; source-only, unvalidated.
- Convert palette route visibility (2026-11-01): `OperationTargetCompositionKind::ConvertPalette` now declares every supported cross-mode P/PA target with its exact source/target identities, prepared source strategy, output mode, scope, and wrapper-owned palette output. `Image::convert` reaches this same declaration through the target-composition dispatcher; the duplicate scope-only P/PA wrapper branch was removed. `prepare_pipeline` remains explicitly TODO because lazy terminals do not yet carry generated palette/alpha metadata. No conversion formula changed; source-only and unvalidated.
- Put wrapper context source audit (2026-11-01): `PutAlpha` and `PutAlphaData` validate and select their mode transition from the owning operation declarations, then enqueue the exact-mode prepared operation; their wrappers do not choose a Backend or Architecture. Immediate PutData/PutPixel writes retain their sequence-scoped resolved context, and ExtractBand/GetChannel routes use the prepared support contract. The former source TODO for wrapper Backend/Architecture bypasses is closed; public promotion/error ordering, CoreOnly outputs, RGBa edge samples, and terminal receipts remain CPU parity TODOs. No validation ran.
- Draw P palette reservation slot alignment (2026-11-02): when `background` or `transparency` reserves the next RGB/RGBA palette slot, the shared allocator now zero-fills skipped entries before writing the new color at the selected index. The full-table reverse scan remains unchanged. Malformed short-tuple behavior is still a source TODO. Source-only; no build, format, test, parity, or lint ran.
- Pad fill representation source trace (2026-11-02; resolved at source level by the following entry): the loss occurred before the kernel when `resolve_imageops_color` narrowed scalar/components into `PipelineOp::Pad`'s byte tuple. The typed route below retains the original input for mode-owned native u16/f32 fills while preserving that direct byte-tuple route. Native wrap/default/alpha semantics and fallback remain pending parity. Source-only; no checks ran.
- Typed Pad fill path (2026-11-02): `ImageOpsColor::Scalar/Components` now retains original integer values in `PipelineOp::PadTyped` for I;16 identities and the five internal high-depth identities. The normalized byte view remains for request facts; exact-mode owners write native u16/f32 fill samples. The previous direct `PipelineOp::Pad` byte-tuple route is unchanged. GPU explicitly declines the typed variant, leaving CPU as its execution owner. CoreOnly modes remain marked `NO_PILLOW_SUPPORT`; mode-specific wrap/default/alpha behavior and fallback still need frozen CPU parity. No validation ran.
- ImageOps Python fill-shape preservation (2026-11-02): `ImageOpsColor` distinguishes Python integer tuples from integer lists at the binding boundary. Mode-specific resolution accepts Pillow's integer RGB(A) list extension only for P and reports list-type errors for other raster modes; Rotate forwards list provenance to its transform fill resolver. F-mode float tuples remain native scalar fills, while float lists retain their list error. No raster or sample conversion was added. TODO: source-review mixed, nonnumeric, and oversized tuple/list components, P palette mutation/error ordering, and remaining native sample defaults; then verify in the frozen CPU parity matrix. No validation ran.
- Composed Resize mode-policy source correction (2026-11-02): exact `One` and `ImagePalette8` strategies now own nearest-filter normalization. The public Resize wrapper and prepared CPU Resize normalize before architecture selection; `Contain`, `Cover`, `Fit`, `Scale`, and `Pad` pass their requested filter through the same prepared path, so their nested selection and execution use the mode-effective filter. A preselected child rejects a filter that does not match its exact mode policy. This closes a source-level path where composed geometry bypassed the wrapper's policy; per-operation/mode/architecture parity and receipts remain pending. No validation ran.
- CoreOnly Histogram native-bin source review (2026-11-02): `LA16`, `RGB16`, `RGBA16`, `RGB32F`, and `RGBA32F` scan their typed samples directly into 256-bin-per-band outputs, with separate masked and unmasked loops and no converted image or staging sample buffer. The user confirmed the implementation should favor the fastest native typed scan while preserving exact parity, without converting modes or staging samples. Source comparison against the pre-refactor path confirmed u16's `(sample + 128) / 257` and f32's normalized clamp/round mapping; `RGB32F` historically returned four bands because the fallback projected to RGBA8, so its native scan now fills the opaque fourth band directly. The same exhaustive mode classifier now exposes the vector result layout as result bands and bins per band. TODO: freeze bin-edge and mask parity and record performance evidence; no checks ran.
- Draw P tuple source TODO (2026-11-02): Pillow 12.2 `_getink` sends any tuple through `ImagePalette.getcolor` before native ink validation; `getcolor` accepts malformed arities and can append/replace raw tuple bytes. The Python boundary currently collapses tuple/list inputs, and core only palette-allocates 3/4-component sequences. Preserve tuple/list provenance and mirror allocation/error ordering in the P palette owner. No behavior change or validation ran.
- Serial CPU scheduling source audit (2026-11-02): active `pool_cpu`, core `ops`, `color`, and `image` code contain no Rayon or parallel-iterator calls; Default and SIMD architecture strategies stay serial. The only compiled scheduler helpers are the opt-in, root-level `par` module with no active callers; move or retire it with the later `pool_parallel` work. No scheduling behavior changed; no checks ran.
- ImageOps Colorize caller reconciliation (2026-11-02): `validate_colorize_mode` now consumes `OperationId::Colorize`'s exhaustive support declaration rather than duplicating its L-only check. The descriptor preserves Pillow's empty-message `AssertionError` for every rejected mode and the wrapper still validates before color-parameter ordering. Source-only; no checks ran.
- ImageOps Invert wrapper route (2026-11-02): added `OperationId::ImageOpsInvert` as the public wrapper contract, linked to the prepared `Invert` child. Its four Pillow wrapper rejections are declared at `WrapperGuard`; modes rejected by the child remain queued and are reported at `ChildAdmission`, preserving the existing lazy error boundary. `invert` and `invert_ops` now consume this route, with the P-specific NotImplementedError still preceding it. Source-only; parity and route-receipt evidence remain pending.
- ImageOps Expand typed fill route (2026-11-02): `ImageOpsColor::Scalar/Components` now retains original i64 values through `PipelineOp::ExpandTyped`; I;16 and CoreOnly LA16/RGB16/RGBA16/RGB32F/RGBA32F owners consume native u16/f32 fills. The direct byte-tuple `PipelineOp::Expand` route is unchanged, and GPU explicitly declines the typed variant. Source review remains unvalidated; native wrap/default/component behavior, architecture fallback receipts, and CPU parity remain pending.
- Pad typed fill source review (2026-11-02): the four I;16 identities and five CoreOnly high-depth identities reach exact-mode Pad owners without changing the native sample type; shared `samples::pad` allocates/copies u16 or f32 buffers directly. I;16 casts the original scalar to u16 (low 16 bits), while multiband owners read each retained component and apply their currently declared missing-band defaults; CoreOnly contracts select the Default kernel for non-Default architecture requests. Existing byte-tuple Pad owners and the direct byte-tuple API remain separate. TODO at `geometry/pad/ops.rs` and the plan row still requires parity for wrap, component/alpha defaults, and fallback receipts before this contract is closed. No validation ran.
- CPU module-facade source review (2026-11-02): moved the `CpuPool` facade implementation into `pool_cpu/executor.rs`; moved `RgbPixelSource` and its slice implementations into `helpers/quantization/rgb_source.rs`; split Draw's layout/strategy and span writer out of their `mod.rs` files; moved all 39 exact-mode `Input` aliases into their mode `ops.rs` files and re-exported them; moved Filter's blur bias into `blur.rs` and the i32 NEON lane count into its plan. The maintained CPU `mod.rs` files now contain module declarations and deliberate exports only; legacy archive files remain untouched. The code paths and formulas were preserved. No build, formatter, checkstyle, lint, test, parity, or benchmark ran.
- Geometry orchestration source review (2026-11-02): moved the unchanged Contain/Cover/Fit/Scale dimension and rounding calculations from `geometry/ops.rs` into `geometry/helpers/dimensions.rs`, leaving the family file to select support and compose prepared Resize children. Caller tracing confirms direct Rust ImageOps validates and queues the typed operations; PyO3 and JavaScript delegate to those Rust entry points; lazy materialization injects the routed context before registry preparation; and geometry passes that same context to its exact-mode Resize child. Fit's no-crop branch clones the exact `DynamicImage`, and prepared execution records the nested Resize selection beside the outer geometry selection. Source review is complete; tie-rounding, zero-sized output, validation/error ordering, exact-mode output, and architecture/fallback receipts remain CPU parity TODOs. No validation ran.
- Tall boxed Resize source comparison (2026-11-02): compared the active vertical-first pipeline with archived `execute_resize_boxed`; the 100:1 height threshold, Float32 box boundaries, LA/RGBA premultiply-once → two prepared stages → unpremultiply-once flow, and nearest bypass retain the archived algorithm. The active route skips the archived redundant horizontal pass only when the horizontal mapping is identity. Added a TODO for exact-mode child selection and output parity across CPU Architecture requests; no validation ran.
- Resize/Reduce pending-path annotations (2026-11-02): added source TODOs at each family owner for exact-mode output, native I;16 values, empty/bounds behavior, and architecture/fallback parity still required by the frozen CPU matrix. No validation ran.
- Draw P tuple/list source follow-up (2026-11-02): the Python boundary now preserves tuple versus list provenance in `DrawColorInput`; integer tuples and lists enter distinct core routes, and P tuple colors with 3/4 integer components use palette allocation while lists reach ink validation without allocation. The remaining TODO is non-integer tuple values, malformed tuple arities, and Pillow's allocation/error mutation order. Source-only; no validation ran.
- Draw P palette error-order follow-up (2026-11-02): 3/4-component integer tuples preflight palette slot availability before byte-range conversion, so a full palette's allocation error takes precedence as in Pillow. RGB/RGBA palette owners and the preflight share slot-selection logic. Remaining TODO: non-integer tuple values, malformed tuple arities, and arbitrary-size integer components. Source-only; no validation ran.
- Draw P oversized integer tuple follow-up (2026-11-02): the PyO3 boundary now preserves a 3/4-component Python integer tuple when components exceed i64, using an impossible palette-byte sentinel so it reaches P allocation preflight and the existing byte-range resolver. Other modes retain the prior invalid-tuple route. Remaining TODO: non-integer tuple values, malformed tuple arities, and Python palette color-map mutation when raw-byte construction fails. Source-only; no checks ran.
- Draw P float tuple follow-up (2026-11-02): all-float 3/4-component tuples retain their float values through the Python boundary. P palette resolution compares them numerically with existing byte entries, normalizes RGB/RGBA tuple arity and opaque alpha, and preflights allocation before reporting the `bytes(float)` error for a new color. Mixed/non-numeric tuples, nonstandard arities, and Python palette color-map mutation after allocation errors remain TODOs. Source-only; no checks ran.
- Draw P mixed numeric tuple follow-up (2026-11-02): the Python boundary now retains integer, wide-integer, and float component kinds for mixed numeric tuples. P palette lookup compares normalized numeric values first; on a miss it checks slot availability, then reports the first `bytes(tuple)` range/type failure in component order. RGB opaque-alpha truncation and RGBA alpha extension are preserved, and an all-integer effective tuple reuses the existing integer palette writer. Remaining TODOs: non-numeric tuple components, other tuple arities, and Python `ImagePalette.colors` mutation after failed raw-byte construction. Source-only; no validation ran.
- Histogram contract confirmation (2026-11-02): CoreOnly histogram work should use the performance-oriented native typed scan while preserving the archived bin contract exactly; do not convert modes or stage another sample buffer. The current LA16/RGB16/RGBA16/RGB32F/RGBA32F owners implement that plan. Boundary, mask, and performance evidence remains gated; no checks ran.
- Draw reserved-index source correction (2026-11-02): rechecked Pillow 12.2 `ImagePalette._new_color_index` and `getcolor`. Pillow advances the returned index over reserved background/transparency values, then appends `bytes(color)` at the palette's current byte length without inserting zero entries. Removed the maintained RGB/RGBA allocator's zero-padding, corrected the Draw plan, and superseded the earlier zero-fill claim in this inventory. Verify reserved-slot outcomes in frozen CPU parity; no tests or lints ran.
- Palette float source repair (2026-11-02): restored the RGB/RGBA palette-byte read in `prepare_palette_float_components`, which the PA capacity branch uses after checking existing float-equal entries. This fixes a source-level missing local found during review; no build or tests ran.
- Python Draw palette lookup source review (2026-11-02): added Pillow-compatible `ImagePalette.colors` caching and `getcolor` semantics, including arbitrary tuple arities, RGB/RGBA alpha normalization, dictionary key equality, slot selection, and map mutation before `bytes(color)` errors. Rust Draw now consumes up to 256 complete entries without replacing the Python-owned byte buffer; `_sync` updates only the raster handle, so trailing bytes remain visible through the shared ImagePalette object. Strict palette realization remains at load/access boundaries. `ImagePalette.getdata()` and `tobytes()` now preserve Pillow's rawmode contract. The remaining source TODOs cover rawmode/LA/PA transparency realization and routing all load-triggering wrapper operations through the deferred boundary. The direct JS/Rust resolver now treats numeric arrays as tuple-like P colors at every arity, retains normalized color keys before byte-conversion errors, and checks allocation before key insertion. Valid nonstandard arities update complete core entries; incomplete raw palette tails remain TODO because the indexed representation retains only complete RGB/RGBA entries. Source references: [Pillow 12.2.0 Image.load](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L896-L925), [Pillow 12.2.0 ImageDraw._getink](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py#L135-L158). No build, formatting, lint, test, parity, or benchmark ran.
- ImagePalette API follow-up (2026-11-02): removed a duplicate `tobytes()` definition that shadowed the rawmode rejection and restored Pillow's `tostring` alias. `copy()` now copies the underlying palette sequence while leaving the derived `colors` cache uninitialized, matching Pillow's copy semantics. Remaining deferred-load and LA/PA transparency behavior is still source review work; no checks ran.
- Deferred palette load-boundary continuation (2026-11-02): added realization before Python wrapper calls whose Pillow implementation loads immediately: copy, split, filter, pixel statistics/readers, histogram/entropy source and mask, channel extraction, putdata/putalpha, point, bitmap export, and public Draw construction/bitmap masks. This preserves Pillow's load-before-validation order for filter and Draw initialization. The boundary TODO now names the remaining argument-validated operations whose realization placement must follow their exact validation sequence, plus rawmode and LA/PA transparency unpacking. No builds, formatters, lints, tests, parity, or benchmarks ran.

- CoreOnly Histogram conversion-equivalence review (2026-11-02): compared the direct u16/f32 bin formulas with the existing byte projection conversions. u16 uses the same rounded `(sample + 128) / 257` mapping as `FromPrimitive<u16> for u8`; f32 uses the same normalized clamp, NaN-to-upper-endpoint, and round behavior as `FromPrimitive<f32> for u8`. The implementation computes bins directly from native samples and keeps separate masked/unmasked loops. Boundary, mask, and performance evidence remains pending; no checks ran.
- Histogram terminal receipt boundary (2026-11-02): source and optional mask may each materialize a lazy graph before the terminal CPU scan. The current eager telemetry initializer clears operation samples, so the scan cannot safely add its selection without losing the materialization receipts. The Histogram owner TODO now requires a scoped collector that preserves source/mask receipt order and appends the terminal selection. This is instrumentation-only; native scan behavior is unchanged and no validation ran.
- RemapPalette source review (2026-11-02): compared archived inverse-map semantics with the exact L/P owners, public wrapper validation and palette-alpha remapping, fixed P output mode, empty-raster path, and Default/SSE2/NEON/Wasm selection. Removed two unreferenced per-mode Default wrappers that duplicated the shared native remap helper. CPU parity and receipts remain pending; no checks ran.
- Convert-to-LAB I;16 receipt correction (2026-11-02): source tracing found the composed I;16→L→RGB→LAB path recorded the admission and later children but omitted the selected I;16→L strategy receipt. Added that receipt immediately before the child executes. Pixel algorithms and outputs are unchanged; runtime receipt parity remains pending and no checks ran.
- I;16 Convert pair scope review (2026-11-02): Pillow 12.2's conversion table supports `I;16`→L/RGB/I/F, `I;16L` and `I;16B`→L/I/F, and `I;16N`→L; exact same-mode identities copy directly. The four Convert mode declarations now classify every other implemented target pair as CoreOnly, including palette targets and cross-identity I;16 conversions, while preserving the existing algorithms. `NO_PILLOW_SUPPORT` comments mark the Rust extensions. Source: [Pillow 12.2.0 Convert.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L1463-L1486). CPU parity and selection receipts remain pending; no checks ran.
- I;16 Convert alias output correction (2026-11-02): CoreOnly conversions among `I;16`, `I;16L`, `I;16B`, and `I;16N` now preserve the native u16 samples instead of clipping through 8-bit luma. These variants share the same typed carrier; byte order is applied at image I/O boundaries. TODO: verify all 4×4 source/target aliases with samples above 255 and mode-specific byte export in CPU parity. Source-only; no checks ran.
- I→I;16N Convert support correction (2026-11-02): Pillow 12.2 has no `IMAGING_MODE_I`→`IMAGING_MODE_I_16N` converter, while the maintained I32 strategy already implements this target as a Rust extension. `ImageI16N` is now declared CoreOnly for I32 instead of being counted as Pillow support. Since all I;16 identities share numeric u16 semantics, I32→I;16N now preserves the clamped u16 sample just like the I;16/I;16L/I;16B targets; only output byte order differs. TODO: add target-only CPU parity for negative, above-u16, above-255, and in-range values. Source: [Pillow 12.2.0 Convert.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L1563-L1588). No validation ran.
- Convert source-target scope correction (2026-11-02): reconciled the exact-mode `ConvertTarget` declarations with Pillow 12.2 `Convert.c`, including `topalette` and `frompalette` paths. Per-mode contracts now classify unsupported implemented pairs as CoreOnly, including nonidentity LAB, exact I/F/CMYK/YCbCr/HSV and premultiplied pairs; `RGB→La/RGBa`, `LA→La`, and `RGBA→RGBa` remain Pillow-supported. RGBX target compositions use the direct Pillow source map so I;16/YCbCr/HSV-to-RGBX fallbacks are CoreOnly. This supersedes the earlier plan wording that incorrectly marked RGB→La/RGBa as CoreOnly. No pixel algorithms changed. Source: [Pillow 12.2.0 Convert.c converters](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L1372-L1486), [palette routes](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L1028-L1105). Remaining architecture-route and receipt audit, parity, and selection evidence are pending; no checks ran.
- Convert annotation correction (2026-11-02): removed stale `NO_PILLOW_SUPPORT` labels from supported `RGB→La` and `RGB→RGBa` direct kernels. The composed Rust-only premultiplied route now marks only its unsupported fallback pairs. Pillow behavior and support declarations are unchanged; no checks ran.

- Histogram receipt continuation (2026-11-02): the wrapper now captures operation receipts after source materialization and again after optional mask materialization, preserving source-before-mask validation order, then appends the terminal prepared Histogram selection. The terminal summary currently reports only the CPU scan timing/resources; aggregation across lazy source and mask graphs is an explicit TODO in the plan and source. CoreOnly u16/f32 histogram binning remains direct over native samples with no mode conversion or staging buffer. No builds, formatters, lints, tests, parity, or benchmarks ran.
- Entropy receipt continuation (2026-11-02): Entropy now captures lazy source and optional mask operation receipts separately, restores them in source-before-mask order, and appends the prepared Entropy selection after its Histogram child finishes. Mask validation still occurs after mask materialization and retains dimension-before-mode ordering. The summary only measures the terminal CPU scan; aggregate timing/resources remain TODO. No builds, formatters, lints, tests, parity, or benchmarks ran.
- GetBBox receipt continuation (2026-11-02): the direct wrapper now preserves source graph operation receipts and appends the prepared exact-mode/architecture selection after materialization. Existing CPU context routing and empty-image selection behavior remain in the same prepared owner. The summary reports the terminal CPU scan only; graph timing/resource aggregation is a TODO in the plan and source. No builds, formatters, lints, tests, parity, or benchmarks ran.
- Analysis receipt continuation (2026-11-02): unmasked/masked Stat and GetColors now capture lazy source (then mask for masked Stat) operation receipts and append their prepared terminal selections. Stat still validates mode before materialization; masked Stat materializes source, then mask, then validates mask dimensions/mode. Terminal summaries remain scan-only with timing/resource aggregation TODOs. No builds, formatters, lints, tests, parity, or benchmarks ran.
- Direct analysis receipt continuation (2026-11-02): formatted GetExtrema, byte-valued GetExtrema, GetBBox, and positive-limit GetColors now capture lazy source graph receipts and append their exact-mode prepared selection. Unmasked Stat does the same; masked Stat captures source then mask receipts and appends its prepared selection, retaining mode-before-materialization and mask size-before-mode validation. All these terminal summaries report only the direct CPU operation; aggregate source/mask timing and resource semantics remain TODOs in the plan/source. No builds, formatters, lints, tests, parity, or benchmarks ran.
- GetChannel/Split receipt continuation (2026-11-02): first-time lazy source materialization receipts are now retained for GetChannel identity-copy routes and Split identity/copy routes, followed by the native identity/copy receipt. Split multi-band results retain source graph receipts while deferring ExtractBand selection until each returned child is used. Summary timing/resources still cover only the terminal native wrapper operation where present; aggregate source-graph summary semantics are TODO. No builds, formatters, lints, tests, parity, or benchmarks ran.
- GetChannel source decomposition (2026-11-02): split the public wrapper's cached identity-copy, shared-buffer identity, tiny-RGB eager child, and materialized child orchestration into focused helpers so the operation entry point stays small. Preserved validation before materialization for rejected modes, materialization before index validation on lazy sources, the existing route guards, metadata propagation, child fallback, and receipt capture order. CoreOnly Histogram remains the selected native typed scan with no image conversion or sample staging; u16/f32 binning is source-equivalent to its archived byte mapping. No builds, formatters, lints, tests, parity, or benchmarks ran.
- GetData lazy receipt continuation (2026-11-02): full-buffer and scalar-only band reads now collect materialization receipts before appending the terminal GetData selection; formatted I/F/I;16 reads do the same. Caller-owned CPU band writes preserve source graph receipts before the terminal selection, while SIMD/GPU band projection retains its selected ExtractBand child receipt. Validation still precedes route execution, and source mode/error ordering is unchanged. Terminal summaries remain host-read-only; aggregate source graph timing/resource semantics are TODO. No builds, formatters, lints, tests, parity, or benchmarks ran.
- Split pipeline branch source update (2026-11-02): after Split materializes a multiband parent to establish the exact band count, non-palette mode-changing pipelines now branch ExtractBand children from the resolved pixels instead of replaying the parent graph for each band. P/PA keep the palette-aware pipeline boundary. Explicit backend pins are reapplied to each child, and ExtractBand selection remains deferred until child use. Added parity/performance TODO coverage for output identity, metadata, selection receipts, and parent replay. No builds, formatters, lints, tests, parity, or benchmarks ran.
- Split shared-materialization update (2026-11-02): Split now obtains the exact logical band count from `materialized_shared()` instead of cloning the full raster through `materialize()`. The cached buffer is reused for child branching. No build, formatter, checkstyle, lint, test, parity, or benchmark ran.
- Enhancement source review closure (2026-11-02): Brightness, Contrast, ColorSaturation, and Sharpness support declarations, native mode routes, exact output/rejection behavior, architecture owners, composition children, and public wrapper context propagation are source-accounted. Default and vector byte Brightness use the same float32 blend helper; the factor `0.09999999999999999` stays a required parity case. Remaining enhancement TODOs are runtime evidence: operation × mode × architecture parity, selection/fallback receipts, and Color metadata behavior. No implementation-only TODO remains in the enhancement owners; no checks ran.
- Python Put/Get load-order source review (2026-11-02): Pillow 12.2 calls `load()` before `getdata()` band validation and uses `_ensure_mutable()` before Python input coercion in `putalpha()` and `putdata()`. The Python facade now crosses that load boundary before its direct Rust/PyO3 calls, so lazy decode and dirty-palette errors precede band/value validation as in Pillow. Remaining TODO: verify load-before-coercion, error order, and retained metadata in the frozen CPU parity cases. No build, formatting, lint, tests, parity, or benchmarks ran.
- Quantize lazy receipt continuation (2026-11-02): direct and explicit-palette Quantize now capture source graph operation receipts after materialization, then restore them before the terminal prepared Quantize selection is recorded. The explicit palette image contributes metadata only and is not separately materialized. Terminal summary timing/resources cover Quantize alone; source-graph aggregation, operation × mode × architecture parity, and receipt evidence remain TODO. CoreOnly histogram remains a native typed scan into the established bin contract, as confirmed by the user; no casts or staging buffers for reuse. No build, formatting, lint, tests, parity, or benchmarks ran.
- Dirty LA palette transparency realization (2026-11-02): after installing a dirty LA raw palette, Python commits its native alpha even when `info["transparency"]` is absent; when present, it applies integer transparency through the Rust palette-alpha index setter or a C-contiguous bytes-like table through the Rust prefix setter. The Rust sidecar uses 256 opaque-default entries, retains raw LA alpha outside the overwritten table prefix, and clears only the codec-side pending marker; Python's public `info["transparency"]` is retained by rebasing its native comparison snapshot before transparency coercion, preserving Pillow's palette-mutation order if the setter errors. Pillow 12.2 `_putpalettealpha(s)` accepts indices/tables up to 256 slots, independent of retained color-entry count. PA raw-palette construction remains a TODO: Pillow's `ImagingPaletteNew` only admits RGB/RGBA/CMYK palettes, so no valid PA palette identity is currently represented. Sources: [Pillow 12.2.0 Image.load](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L3690-L3712), [palette alpha setters](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L1724-L1770), [palette constructor](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Palette.c#L21-L44). Runtime parity and malformed-buffer/error-order verification remain pending; no build, format, lint, test, parity, or benchmark ran.
- PA raw-palette reachability correction (2026-11-02): PA remains a supported raster mode, but it is not a supported raw palette input. Pillow's palette path promotes alpha-bearing raw palette data to RGBA and has no PA-to-RGBA unpacker, so `putpalette(..., rawmode="PA")` remains an explicit `unrecognized raw mode` rejection in core. This supersedes the immediately preceding inventory note that attributed the gap to `ImagingPaletteNew` rejecting PA palette identities. Sources: [Pillow 12.2.0 Image.putpalette](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py), [Pillow 12.2.0 Unpack.c alpha-mode unpackers](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Unpack.c#L1493-L1512). No implementation TODO remains for this rejected raw-mode path; wrapper load-boundary review and parity evidence remain pending. No build, format, lint, test, parity, or benchmark ran.
- Dirty-palette load boundary continuation (2026-11-02): Python `convert`, `quantize` source, and `transpose` now realize dirty palettes at Pillow's load-first boundary. `crop` reproduces Pillow's two reversed-coordinate checks before realization, then delegates to the native crop path. `resize` remains pending because Pillow validates filter, reducing gap, size, and identity before loading; `quantize`'s optional palette image has a separate later load after source method checks. The source TODO now names only unhandled wrappers and that optional reference-palette boundary; `alpha_composite` was removed because its accepted LA/RGBA modes cannot carry a P/PA dirty palette. No build, format, lint, test, parity, or benchmark ran.
- Dirty-palette operation-order continuation (2026-11-02): `quantize` now realizes a dirty source palette at Pillow's source-load boundary and realizes a dirty reference Image palette only after the RGBA method restriction, before reference/source mode errors. `reduce` mirrors Pillow's scalar-factor normalization, default box, identity-copy boundary, then commits a dirty P/PA palette before non-identity execution. `remap_palette` now rejects non-L/P modes before processing and realizes a dirty P palette when Pillow loads it to retrieve the source palette; explicit `source_palette` still has a later load after destination-map processing and remains a source TODO. Together with the preceding wrapper entry, convert/transpose/crop/reduce/quantize and the default remap path now have explicit load boundaries. Save, paste, thumbnail, transform, explicit-source remap, and runtime parity remain pending; resize and Rotate source boundaries are implemented. No build, format, lint, test, parity, or benchmark ran.
- Histogram source-route review (2026-11-02): traced `Image::histogram_with_mask` from shared materialization through size-before-mode mask validation, image-derived `Mode`, injected CPU/Architecture context, `HISTOGRAM_SUPPORT` selection, exact `DynamicImage` dispatch, and the mode-owned Default implementation. Per user decision, CoreOnly u16/f32 modes retain native typed scans even when this prevents code sharing; do not convert samples or stage a projected raster. `RGB32F` declares its existing four-band result and writes the opaque final band directly. No routing or conversion gap was found by source inspection. Frozen bin-edge/mask parity, selection/fallback receipts, graph timing aggregation, and performance evidence remain pending; no build, format, lint, test, parity, or benchmark ran.
- Dirty P/PA resize source continuation (2026-11-02): dirty indexed palettes now use the core resample parser before the Python reducing-gap comparison, then normalize the default box and size tuple and evaluate Pillow's identity condition before committing palette bytes. The identity route canonicalizes geometry and drops a valid reducing gap as Pillow's `copy()` path does, while still calling Rust resize so its injected backend/architecture strategy and receipt remain intact. Non-dirty image resize calls are unchanged. Source-only against Pillow 12.2; save, paste, thumbnail, transform, explicit-source remap, unusual dynamic argument parity, and runtime evidence remain pending. Rotate uses a parse-once typed request before palette realization. No build, format, lint, test, parity, or benchmark ran.
- Dirty P/PA Rotate source continuation (2026-11-02): ordinary Rust/Python Rotate parses host values once and calls `rotate_with_input`, while dirty indexed palettes use the prepared-request API to validate before realization and enqueue after palette metadata is committed. The prepared plan records the exact source `Mode` and rejects use with a different mode; no raster or sample conversion occurs. Core Rotate requests with empty sources are routed to each exact mode's Default strategy, which preserves only the archived discrete empty-output fast paths and leaves other geometry on checked Default behavior. Source routing and argument order were reviewed; TODO: verify dirty-palette mutation/error order, empty outputs, architecture fallback receipts, and mode × Architecture parity in the frozen CPU matrix. No build, format, lint, test, parity, or benchmark ran.
- CoreOnly LA16 Rotate fill review (2026-11-02): `ImageLumaA16` uses the native two-sample `(luma, alpha)` carrier, so Rotate now maps the public fill tuple's alpha component into the native helper's fourth fill slot before its direct u16 path. No intermediate mode or byte raster is used. CoreOnly fill parity remains in the frozen CPU extension cases; no verification ran.
- I;16 Rotate empty fast-path source review (2026-11-02): exact I;16 owners now allow zero-sized output only for the archived 180° and expanded 90°/270° discrete paths. Non-expanded square quarter-turns retain checked allocation. This matches the archived image-crate right-angle calls without changing I;16's native-u16/floating-affine route; empty-output and fallback receipts remain in CPU parity. No verification ran.
- ModeFilter strategy source continuation (2026-11-02): the prepared plan now binds one exact `DynamicImage` variant to a closed mode strategy before execution. Its shared algorithm accepts a typed byte buffer and const channel count, contains no runtime mode switch, returns the same exact variant, and reuses fixed stack histograms instead of allocating a histogram vector for every output pixel. The P wrapper still restores palette metadata around the native-index result. Pillow mode/palette parity, fallback receipts, and output checks remain pending; no verification ran.
- ModeFilter support scope correction (2026-11-02): Pillow 12.2 C accepts only one-band uint8 rasters and copies the palette on output; `One`, `L`, and `P` retain Pillow scope. The native per-channel LA/La/PA/RGB/RGBA/RGBa/RGBX/CMYK/YCbCr/LAB/HSV strategies are now explicitly `CoreOnly` with nearby `NO_PILLOW_SUPPORT` comments. Source: [Pillow 12.2 ModeFilter.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/ModeFilter.c). Parity and selection receipts remain pending; no checks ran.
- Filter Python C-int boundary (2026-11-02): Max/Min/Median/Mode/Rank adapters now receive the original Python integer object and call the shared `inputs::c_int_from_python` parser, which uses PyLong/C-long conversion followed by C `int` bounds. This matches Pillow's `_imaging` `i`-argument parsing without an intermediate `i64` narrowing. PyO3 signatures carry Pillow's numeric defaults, so omitted arguments default while explicit `None` reaches the same integer conversion path as Pillow. The out-of-range messages follow CPython's signed-int argument parser; negative size/rank behavior remains operation-specific. ImageChops now reuses the same parser without changing its conversion contract. Rank-family source validation now matches Pillow 12.2's odd/zero size errors and float32 workspace overflow limit, returning `filter size too large` before the native kernel path. Sources: [Pillow 12.2 ModeFilter `i` parser](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L1349-L1357), [Pillow 12.2 RankFilter `ii` parser](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L1807-L1815), [Pillow 12.2 rank workspace guard](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/RankFilter.c#L67-L79), [CPython 3.12 C-int parser](https://github.com/python/cpython/blob/3.12/Python/getargs.c#L688-L705). TODO: parity for omitted versus explicit `None`, bool and `__index__` inputs, C-long overflow, signed-int edges, window-limit error ordering, filter-name/mode/error ordering, and selected/fallback receipts. No build, formatter, lint, test, parity, or benchmark ran.
- Draw Bitmap/Text source continuation (2026-11-02): Pillow 12.2 `ImagingDrawBitmap` delegates to `ImagingFill2`; `fill_mask_RGBa` reads mask alpha and applies PREBLEND, so the archived Rust note that RGBa Bitmap masks are opaque is stale. The maintained generic and RGB-specialized owners both use alpha/PREBLEND. For I;16, the 1/RGBA/RGBa fill routines advance through the raw row byte prefix at one byte per mask pixel; L blends both stored bytes at each sample stride. The exact-mode helper comments now state those raw-byte bounds, and Bitmap indexing widens dimensions before multiplication to avoid u32 index overflow. I;16 byte-pattern parity, Text ink and palette/error order, Bitmap fast-route metadata, and operation × mode × architecture receipts remain TODO. No build, format, checkstyle, lint, test, parity, or benchmark ran.
- Bitmap binding load-order continuation (2026-11-02): public Python `ImageDraw.bitmap` already loads the mask before palette/fill resolution; the lower-level PyO3 entry now calls the existing `PyImage.load` method before parsing `fill`, preserving the same order for direct extension calls and releasing the GIL through the established loader. This duplicates only a no-op load on the normal facade route. Error-order and mutation parity remain pending; no build, format, checkstyle, lint, test, parity, or benchmark ran.
- Draw Text error-order continuation (2026-11-02): Pillow 12.2 builds `ImageText.Text` (including the omitted default font) before embedded-color validation and ink resolution, and only resolves `stroke_fill` when stroke width is truthy. The Python facade and direct PyO3 entries now prepare the default font first, reject invalid embedded-color modes before palette fill mutation, and skip unused stroke colors; the facade exposes keyword-only `font_size`, and default font/cache handling follows Pillow. Existing `ImageText.Text` objects and arbitrary custom-font/extra-argument behavior remain TODO. Sources: [Pillow 12.2 ImageDraw.text](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py#L3103-L3205), [Pillow 12.2 ImageText.Text](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageText.py#L1332-L1455). Palette/error-order parity remains pending; no build, format, checkstyle, lint, test, parity, or benchmark ran.
- Draw Bitmap Paste-child support link (2026-11-02): the direct Draw support declaration now links the prepared Paste contract used by the compatible accelerated Bitmap route. The route still needs a declarative account of its exact destination/mask/context/backend/nonempty guards and whether its lazy execution receipt is identified as Draw or Paste; preserve this as TODO. No build, format, checkstyle, lint, test, parity, or benchmark ran.
- Draw Bitmap Paste-route guard inventory (2026-11-02): source-traced `bitmap_masked_paste_matches`: a nonempty `1` mask delegates for same-mode contexts `1`, `P`, `L`, `LA`, `RGB`, `RGBA`, `CMYK`, and `HSV`; a nonempty `L` mask delegates for same-mode `L`, `RGB`, `CMYK`, and `HSV`; RGBA draw context over RGB storage is additionally admitted for RGB-compatible masks. The wrapper additionally requires an explicit non-CPU backend request. The support link names Paste, but does not yet encode these conditional guards or the Draw-vs-Paste receipt identity. No validation ran.

- Draw conditional-route support continuation (2026-11-02): Draw's top-level `OperationModeRoute` now exposes the accelerated Bitmap-to-Paste child per exact mode with raster/context/mask triples, GPU/SIMD request, and nonempty-mask guards; runtime selection evaluates that declaration instead of a separate boolean mode matcher. The wrapper queues `PipelineOp::Paste`, so the lazy child receipt is Paste by source inspection. Fast-route parity and receipt evidence remain pending. CoreOnly Histogram stays on direct native typed sample scans, without cross-mode casts or staging buffers; optimize each typed owner for parity and measured latency. No build, format, checkstyle, lint, test, parity, or benchmark ran.
- Python PutPixel load boundary (2026-11-02): Pillow 12.2 `Image.putpixel` calls `_ensure_mutable()` before coordinate or value parsing; `_ensure_mutable()` loads the image even when it is already writable. The Python facade now calls its `load()` boundary before entering the internal PyO3 mode-aware writer, so deferred decode/pipeline errors occur before pixel coercion and the mutation is visible at the Pillow call boundary. This also realizes a pending dirty palette using the same loader path as other public mutators. Source review only; PutPixel ordering, metadata, and receipts remain pending frozen CPU parity. Source: [Pillow 12.2 `Image._ensure_mutable`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L668-L677). No build, formatting, checkstyle, lint, test, parity, or benchmark ran.
- Python GetPixel/GetChannel boundaries (2026-11-02): Pillow 12.2 loads before `tuple(xy)`/core GetPixel parsing and before GetChannel selector handling. The facade now uses that ordering; the GetPixel binding accepts signed C-int coordinates and returns IndexError for negative values instead of failing unsigned extraction. Source-only; input coercion, error, and receipt behavior remain pending frozen CPU parity. Sources: [Pillow 12.2 GetPixel](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L1612-L1625), [Pillow 12.2 GetChannel](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2563-L2581). No build, formatting, checkstyle, lint, test, parity, or benchmark ran.

- GetPixel negative-coordinate correction (2026-11-02): Pillow 12.2 `_imaging.c` adds image width/height to negative `getpixel` and `putpixel` coordinates before bounds checks. The prior source note incorrectly said negative GetPixel coordinates should raise immediately; the PyO3 binding now shares PutPixel's right/bottom-relative normalization helper before selecting the loaded RGB fast path or formatted core read. CPU parity for negative and out-of-range coordinates, C-int limits, and error order remains pending. Source: [Pillow 12.2 `_imaging.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L487-L505). No build, formatting, checkstyle, lint, test, parity, or benchmark ran.

- Pillow I/F Histogram extrema continuation (2026-11-02): the Python facade now forwards optional `extrema` through the binding into the exact `I`/`F` native scan. The support declaration exposes `HistogramExtrema` only for those mode identities. Without a mask, the scan uses Pillow-compatible float32 scaling, truncating bin conversion, and omits out-of-range bins; a mask or a mode outside I/F ignores the optional extrema as the Pillow wrapper does. CoreOnly u16/f32 scans remain direct native typed scans without conversion or staging. Error-order and boundary behavior remain pending CPU parity; no checks ran.

- Entropy range API preservation (2026-11-02): retained the existing optional Python/Rust `entropy(..., extrema=...)` parameter and routed it through the native I/F Histogram child for unmasked inputs. The parameter is marked `NO_PILLOW_SUPPORT` because Pillow 12.2 Entropy is mask-only; the default no-extrema path remains the parity path. Masked Entropy ignores the range. No checks ran.

- Python read-boundary continuation (2026-11-02): `split`, `getextrema`, `getprojection`, and `getpalette` now call the shared `load()` boundary before their native read, matching Pillow 12.2 load/dirty-palette realization. This also captures pending pipeline/decode errors before those direct scans. Source-only; load/error/receipt parity remains pending and no checks ran.

- CMYK PutAlpha scalar-owner refinement (2026-11-02): Default PutAlpha and PutAlphaData now apply the shared rounded CMYK pixel conversion directly into their required RGBA output, removing the temporary RGB raster while preserving arithmetic and the separate architecture-specific vector owners. Source-only; parity is still pending. No checks ran.

- Palette PutPixel reserved-index source continuation (2026-11-02): traced the P/PA palette allocator against Pillow 12.2 `ImagePalette._new_color_index` and `getcolor`. Python PutPixel supplies the current public `Image.info` reserved indices as authoritative; core PutPixel derives them from retained compatibility metadata. Palette-alpha storage no longer independently reserves index 0 after public metadata changes. For short palettes, Pillow may return a later reserved-safe index while appending new bytes at the current palette end; the Rust path preserves that observable behavior. Frozen parity remains TODO for reserved holes, full-palette reuse, metadata mutation, and error ordering. Source: [Pillow 12.2.0 ImagePalette.py](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImagePalette.py#L106-L169). No build, format, checkstyle, lint, test, parity, or benchmark ran.

- Palette PutPixel float-tuple source continuation (2026-11-02): exact-mode palette writes now normalize RGB/RGBA tuple arity before matching existing palette entries, reject nonopaque RGBA values for RGB palettes, and check slot allocation before returning Pillow's float-to-byte error. P uses reserved-aware unused-slot selection; PA retains its no-reuse limit. Nonstandard tuple arities and the Python palette color-cache state after `bytes(color)` errors remain TODO. Source: [Pillow 12.2.0 ImagePalette.py](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImagePalette.py#L106-L169). No build, format, checkstyle, lint, test, parity, or benchmark ran.
- PutPixel C-int coordinate parsing (2026-11-02): coordinate components now use the shared PyLong-to-C-int parser instead of narrowing an `i64` with `as i32`, so oversized Python integers raise the C argument overflow error rather than wrapping to another pixel coordinate. Pillow 12.2's `_imaging.putpixel` parses `(ii)O` before checking negative-relative coordinates and bounds. Signed C-int edge, coercion, and error-order parity remain pending. Source: [Pillow 12.2.0 `_imaging.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c). No build, format, checkstyle, lint, test, parity, or benchmark ran.
- CoreOnly f32 Histogram bin exactness (2026-11-02): native f32 scans now use the same fused `mul_add(255.0, 0.0)` followed by rounding as the archived `FromPrimitive<f32> for u8` projection. This preserves projection-edge bins while reading each native sample directly and creating no converted raster or staging sample buffer. NaN/clamp/mask boundary parity remains pending; no checks ran.

- Native F32 Pad/Expand fill continuation (2026-11-02): `NativeColorInput` now preserves float scalars and tuple components as f64 bit values alongside integer inputs. `ImageOpsColor` routes those values through typed Pad/Expand operations, and the exact RGB32F/RGBA32F CPU Default kernels read the native value and convert once to the destination f32 sample. The kernel does not consume a byte-projected float fill. Integer fills still convert directly to the destination f32 sample. Native fill parity, alpha/default behavior, architecture fallback receipts, and composed Resize remain pending; no build, formatting, checkstyle, lint, test, parity, or benchmark ran.

- I;16 Histogram row-prefix source reconciliation (2026-11-02): Pillow 12.2 `Storage.c` aliases row pointers through `image8` for two-byte pixels, and `Histo.c` reads `image8[y][x]`. The result counts the first `width` raw bytes of each row and restarts byte-lane selection per row, including odd widths. The maintained exact-mode owner reconstructs each requested byte from the native u16 carrier with a compile-time endian choice and applies mask selection at the corresponding pixel; it does not build a byte raster. Source behavior is resolved; freeze endian, odd-width, and masked cases in CPU parity. Sources: [Pillow 12.2.0 Histo.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Histo.c), [Pillow 12.2.0 Storage.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Storage.c). No build, format, checkstyle, lint, test, parity, or benchmark ran.

- I;16 bytes PutData partial-write correction (2026-11-02): Pillow 12.2 `_putdata` copies each supplied row prefix into the existing raw I;16 row; bytes beyond the copied prefix stay unchanged. The former maintained path zeroed a replacement raw frame and therefore erased untouched sample bytes. The exact-mode Default writer now updates native u16 byte lanes in place, uses the Python-reported entry count, applies scaled-byte clipping, and preserves mode-specific endian order without an intermediate frame. Reads beyond a bytes allocation are capped safely, with zero used for additional undefined entries; nonfinite/out-of-range C casts and the frozen partial/full-row cases remain recorded TODOs. Source: [Pillow 12.2.0 `_putdata`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L1531-L1551). No build, format, checkstyle, lint, test, parity, or benchmark ran.

- Expand/Pad typed-fill source review (2026-11-02): Pillow 12.2 I;16 fill parsing writes the scalar low two bytes, and mode-specific accessors preserve the logical u16 sample; the CPU fill remains typed and wraps directly in that sample owner. ImageOps.pad creates its canvas with Image.new and copies an attached source palette to the result. ImageOps.expand uses the attached P palette, or an empty RGB palette when none is attached, for tuple and named fills; the Rust path resolves existing entries and appends native palette bytes without reusing unused indices or staging pixels, then attaches the resulting palette metadata to the output. Allocation/error-order, dirty Python palette, and malformed palette parity remain pending. No checks ran. Source: [Pillow 12.2.0 ImageOps.py](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py).
- ImageOps.expand asymmetric border source update (2026-11-02): the wrapper and `PipelineOp::Expand` now retain signed left/top/right/bottom margins; the Python adapter accepts scalar, two-value horizontal/vertical, and four-value margins. Exact-mode Default kernels use the symmetric row-copy path when possible and a native-sample row-copy path for asymmetric or negative margins. SIMD selection requires a symmetric nonnegative margin. GPU admission declines other shapes; automatic routing can select CPU, while an explicit GPU request remains strict. TODO: freeze scalar, two-value, four-value, negative-crop, native-mode, and backend-fallback parity cases; review arbitrary-precision values, custom __index__/arithmetic, output-size C-int overflow, and color/error-order parity. No checks ran. Source: [Pillow 12.2.0 ImageOps.expand](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L2462-L2514).
- ImageOps.expand P palette continuation (2026-11-02): the Rust operation wrapper now resolves P-mode tuple and named colors against a palette snapshot, preserving RGB/RGBA palette mode and existing-index lookup, allocating by append without pixel-table reuse, and attaches the updated palette to the lazy result. The P mode support declaration exposes these semantics as `ExpandFillRule::PaletteIndexOrColor`. Palette work allocates no image-sized buffer. The Python ImageOps adapter normalizes borders through the Rust parser before realizing a dirty P/PA palette; Rust then resolves P fills against existing palette metadata or an empty RGB palette and carries the updated palette to the lazy result. TODO: verify dirty/raw palette error ordering, malformed palette inputs, allocation, and exact output metadata in CPU parity. No checks ran.

- P ImageOps fill source correction (2026-11-02): `Image.new("P", ..., color)` uses a temporary RGB palette for integer RGB(A) tuples, and `ImageOps.pad` replaces it with the resized source palette only when one is attached. `ImageOps.expand` resolves names through `ImageColor.getcolor`; P names therefore become RGB tuples with string alpha discarded before palette allocation. `ImagePalette.getcolor` rejects non-opaque tuple alpha before converting RGB components with `bytes()`. Rust P tuple/name resolution now preserves that alpha/range order, uses unclamped RGB values for P names, and reports `bytes must be in range(0, 256)` for out-of-range RGB components. Remaining TODO: carry mixed/nonnumeric/arbitrary-width tuple provenance and review dirty/raw palette ordering. The unpaletted P Pad temporary RGB palette is now attached to the lazy output. No tests or validation ran. Sources: [Pillow ImageOps.py](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L2256-L2283), [Pillow ImageOps.expand](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L2491-L2512), [Pillow Image.py](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2956-L2968), [Pillow ImageColor.py](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageColor.py#L118-L153), [Pillow ImagePalette.py](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImagePalette.py#L139-L168).

- Pad P-palette and dirty-palette source correction (2026-11-02): the P Pad strategy keeps the fill at index 0 and attaches the temporary RGB palette produced by valid named/RGB(A) tuple/list fills when the source has no attached palette; a source palette is preserved by the existing pipeline metadata. Fill validation precedes centering access, matching Image.new-before-paste ordering. The PyO3 ImageOps.pad binding computes contain geometry before Python filter coercion, then validates the filter and resize dimensions in a reusable Rust preflight plan; if filter coercion changes image dimensions, execution resizes the current image to the saved contain dimensions before padding. For dirty P/PA inputs, the Python facade runs this ordered preflight before realizing palette metadata and passes the normalized filter onward. None stays None and Python filter conversion is not repeated. Effectful color/centering conversions that replace the image between contain and paste, public results and fault order for mixed/nonnumeric/oversized tuple values, dirty/raw palette error ordering, and mode/architecture parity remain pending. Source-only; no build, formatter, lint, test, parity, or benchmark ran. Sources: [Pillow ImageOps.pad](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L2256-L2283), [repository Image.resize](../pillow-rs-py/python/pillow_rs/image.py#L360-L390), [repository ImageOps.pad facade](../pillow-rs-py/python/pillow_rs/imageops.py#L69-L78).

- CoreOnly Histogram formula source reconciliation (2026-11-02): compared the native exact-mode owners with the prior committed `ops/analysis/histogram.rs` projection and `raster/color/from_primitive.rs`. u16 bins preserve `(sample + 128) / 257`; f32 bins preserve the `[0, 1]` clamp, NaN/±infinity mapping, fused `mul_add(255.0, 0.0)`, and round operation. LA16/RGB16/RGBA16 retain 2/3/4 bands; RGBA32F retains four; RGB32F preserves its prior synthesized opaque fourth band, including masked pixel counts. These Default owners scan native storage and allocate only the output histogram. Frozen extension parity at bin boundaries, masks, empty inputs, output shapes, and performance remains pending. Source review only; no code validation ran.
- Quantize exact-mode source continuation (2026-11-02): the default quantization method now comes from the exact-mode strategy, with RGBA/RGBa selecting method 2 and RGBX retaining the RGB default. RGBX now reads only its native RGB channels, ignores the unused fourth byte, and emits RGB palette entries; this corrects the earlier inventory claim that RGBX used the alpha-aware octree and had RGBA method restrictions. TODOs remain in Quantize for premultiplied RGBa palette samples and CoreOnly LA/La/PA alpha and palette semantics, CMYK/YCbCr/LAB/HSV palette colors, I/F scalar carriers, and high-depth alpha bands. These paths must keep direct native reads and avoid a converted or relabeled raster. Source-only; no C code, build, formatting, lint, test, parity, or benchmark was introduced or run.
- Quantize alpha-source continuation (2026-11-02): introduced a statically dispatched `RgbaPixelSource` and alpha-aware octree scan. LA, La, PA, LA16, RGBA16, and RGBA32F now read their exact native carriers; La unpremultiplies luma per sample, typed u16/f32 alpha uses the established palette-byte projection, and PA resolves an RGB lookup table and combines palette-entry alpha with per-pixel alpha without building a converted image. The exact-mode Quantize contract now names `method_restricted_modes` and `allowed_methods`; these alpha modes default to method 2, while Pillow RGBA alone keeps its pre-materialization validation boundary. TODOs remain for RGBa's premultiplied palette semantics and CMYK/YCbCr/LAB/HSV and I/F source interpretation. Source-only Rust work; no C code, compiler, formatter, lint, test, parity, or benchmark ran.
- Quantize RGBa source correction (2026-11-02): the exact RGBa Default owner now reads premultiplied native samples and unpremultiplies RGB per pixel with the existing Rust byte helper before alpha-aware octree reduction. Palette RGB is therefore straight-alpha data while palette alpha is retained separately; no DynamicImage conversion or staging raster is created. Remaining Quantize TODOs cover CMYK/YCbCr/LAB/HSV palette semantics and I/F scalar carriers. Source-only Rust change; no C code, compiler, formatter, lint, test, parity, or benchmark ran.
- Quantize I/F scalar-source correction (2026-11-02): the I and F Default owners now decode each little-endian four-byte scalar directly from their raw carriers instead of interpreting scalar bytes as RGB channels. I clamps signed values to 0..255 byte grayscale; F uses the established raster f32-to-u8 projection. The resulting grayscale RGB samples are read in place by the selected quantizer without a converted image or staging buffer. These CoreOnly routes are marked NO_PILLOW_SUPPORT. Remaining Quantize color-space review covers CMYK/YCbCr/LAB/HSV. Source-only Rust work; no C code, compiler, formatter, lint, test, parity, or benchmark ran.
- Quantize color-carrier source audit (2026-11-02): compared the Default readers with the Rust `DynamicImage::to_rgb8()` path. CMYK is carried by four byte lanes whose RGB projection reads the first three; YCbCr/LAB/HSV are carried by three-byte RGB buffers whose projection preserves those values. The exact-mode readers reproduce these palette-source samples directly, without building a converted DynamicImage or staging raster. Removed the resolved source TODO; mode × architecture parity and receipts remain pending. Source-only Rust review; no C code or validation ran.
- Qt indexed color-table mode boundary (2026-11-02): `Image::indexed_color_table` now derives L/P behavior from its exact `Mode` instead of accepting a duplicate mode string. The PyO3 method no longer takes a mode argument, and Python `toqimage()` passes only the image-owned Rust value. Other mode identities still return an empty table. This removes a string-based mode decision from the core API without changing the Python Qt format selection. No validation ran.
- Analysis contract size split (2026-11-02): moved GetBBox support and GetProjection support/prepared execution into `analysis/ops/get_bbox.rs` and `analysis/ops/get_projection.rs`; `analysis/ops.rs` re-exports the existing crate-facing names so current callers remain stable. The family file is now 789 physical lines and both child files remain below the 1,000-line ceiling. This is a Rust-only source change; no C/C++ files were introduced. CPU source review and runtime verification remain pending; no build, formatter, lint, test, parity, or benchmark ran.
- PutAlpha wrapper source audit (2026-11-02): compared the mode-specific support map, prepared PutAlpha/PutAlphaData strategies, Rust wrapper, Python facade, and preserved baseline Rust wrapper. The typed declarations preserve Pillow rejections, declare native CoreOnly extensions, and match each queued output mode; the Python facade loads before coercion and refreshes retained `Image.info` after errors. `putalpha_data` validates destination support first and preserves opaque-alpha promotion when later mask validation fails; valid masks go straight to the prepared operation without a converted staging raster. No further source discrepancy was found. CPU parity remains pending for promotion/error ordering, mask modes, CoreOnly outputs, integer coercion, RGBa alpha edge values, and selected/fallback receipts. No validation ran.
- Convert palette composition source audit (2026-11-02): `OperationSupport` declares cross-mode P/PA targets as `ConvertPalette` wrapper compositions. Public conversion dispatch resolves Backend/Architecture, prepares the exact source-mode Convert strategy, captures its selection, and attaches generated palette, alpha, and wrapper metadata to the returned Image. Same-mode indexed copies preserve existing metadata. A raw lazy cross-mode `PipelineOp::Convert` remains explicitly rejected because that prepared raster result cannot carry the wrapper metadata; the operation comment keeps this path as a TODO rather than claiming it is implemented. Source review only; no validation ran.
- PutData wrapper source audit (2026-11-02): traced the Python facade load boundary, PyO3 sequence classification, exact-mode value kind, too-many-entry validation before value coercion, callback-free built-in bulk path, and callback-preserving per-item path into the native CPU owners. I;16 direct writes use a sequence-scoped routed CPU context and the exact endian owner. Remaining source review: P/PA palette metadata and payload-mode transitions, PutPixel wrapper ordering, and ExtractBand callers/receipts. No validation ran.

- PutPixel wrapper source audit (2026-11-02): traced the Python `load()` boundary, PyO3 mode lookup, P/PA palette allocation before coordinate validation, signed-coordinate normalization and bounds checks, Python value coercion, RGB byte fast path, deferred byte-mode writes, and immediate I/F plus I;16 typed writes. Deferred writes enter the exact-`DynamicImage` `PUT_PIXEL_SUPPORT` declaration; its mode owner prepares the selected architecture strategy from the routed CPU context and executes the corresponding Rust kernel. Ordered batches use a separate mode/architecture contract, with RGB fusion admission retaining its resolved context. High-depth public PutPixel rejections and native I/F/I;16 extensions remain explicit. Frozen CPU parity is still required for mode expansion, palette/error order, coordinates, output identity, and selected/fallback receipts. Source review only; no validation ran.

- Encoded header mode accessor and wrapper cache (2026-11-02; historical pin state superseded): image-slash-star PR [#155](https://github.com/appunni-m/image-slash-star/pull/155) merged as `d10d9c53ab0496c60f0dfb9be532e7f2e4251a11`; its owned and borrowed lazy encoded sources exposed the already inspected codec `ImageInfo.mode` without decoding pixels. At the time of this entry, pillow-rs was pinned to `0.1.2` revision `70190214a0711223302c76ab58e76c097288d80b`, and integration of `0.1.3` was deferred. That pin decision is superseded: `pillow-rs/Cargo.toml` and `Cargo.lock` now pin image-slash-star `0.1.3` to `eee65cbdad44594a09e526b11313c7eb4428732a`; this commit is the codec reference for continued work in pillow-rs, while upstream changes are to merge later. The Pillow wrapper stores a non-optional `Mode` translated at encoded-input construction and does not materialize pixels to answer mode queries. Big-endian TIFF L16 maps to I;16B. Premultiplied LA maps to La as a Rust extension, marked `NO_PILLOW_SUPPORT` because Pillow 12.2 has no matching associated-LA TIFF open-table entry. Materialization checks the decoded codec mode against the header and preserves a compatible exact identity on `DynamicImage`, which is authoritative after decode. TODO: freeze parity for every header mode versus its decoded `DynamicImage` identity and deferred decode failures. Upstream image-slash-star checks cited in the original investigation did not run pillow-rs checks.

- PutData indexed metadata source correction (2026-11-02): public P/PA PutData derives its payload mode from the exact DynamicImage mode, so indexed bytes and palette metadata remain paired. Cross-payload internal operations use `OperationId::PutData`'s `ByPayload` output declaration to decide palette retention. Removed the source-independent P/PA payload shortcut, which could retain an indexed wrapper for a cross-width payload whose declared output is L/LA/RGB/RGBA; same-output payloads preserve their palette. Frozen parity remains pending for P/PA outputs, palette metadata, and cross-payload transitions. Source-only Rust change; no validation ran.

- ExtractBand caller and receipt source audit (2026-11-02): traced the Python load-before-selector boundaries for GetChannel, Split, and GetData; exact-mode wrapper route declarations; one-band identity/copy paths; tiny-RGB eager ExtractBand; deferred per-band child pipelines; and direct caller-owned GetData output buffers. Prepared multiband extraction consumes the source `DynamicImage` and injected CPU Architecture context, with architecture selection owned by each exact-mode strategy. Split materializes a mode-changing parent once before branching multiband children. Direct identity paths record native identity/copy receipts without claiming a kernel; eager and terminal CPU routes retain prepared selections, while lazy child selection occurs on materialization. Remaining TODOs: freeze mode × Architecture output and fallback receipts, verify split child metadata, aggregate lazy source timing/resources, and move GPU terminal eligibility into its support declaration. Source review only; no validation ran.
- Formatted GetPixel all-mode source review (2026-11-02): the wrapper now reads ImageLumaA16, ImageRgb16, ImageRgba16, ImageRgb32F, and ImageRgba32F samples directly from their typed DynamicImage buffers and returns native u16/f32 components instead of projecting through the byte RGBA accessor. All 25 variants now dispatch directly from DynamicImage; byte modes avoid the generic RGBA projection, and the wrapper does not materialize twice. The shared bounds check precedes every typed read. Added NO_PILLOW_SUPPORT and a tracked input-driven parity TODO. Rust source only; no checks ran.
- Convert RGB-to-scalar source audit (2026-11-02): compared the archived SIMD typed lanes with the maintained RGB8 Default and architecture strategies. RGB→I preserves `(19595R + 38470G + 7471B + 32768) >> 16`; RGB→F preserves `(299R + 587G + 114B) as f32 / 1000.0`. The current RGB8 contract exposes SSE2, NEON, and Wasm SIMD128 implementations for both exact targets, while scalar I/F source modes remain Default-only: the archived SIMD byte-layout preflight admits L/LA/RGB/RGBA and aliases, not I/F source carriers. Other Convert source/target pairs and complete architecture guards remain under the existing TODO. CPU parity and selected/fallback receipts are pending; no validation ran.

- Direct JS/Rust Draw P resolver continuation (2026-11-02): numeric JavaScript arrays now retain tuple provenance for every arity; integer, floating, mixed, and wide numeric components route through the core P palette resolver. Draw retains normalized `ImagePalette.colors` keys before byte type/range conversion errors, after allocation preflight, and resolves cached keys on later calls. RGB/RGBA normalization and complete-entry updates are preserved. TODO: the indexed core image cannot retain an incomplete trailing raw palette entry for malformed arities, unlike Python's wrapper-owned ImagePalette. Source-only; no build, formatting, lint, test, parity, or benchmark ran.

- Draw multiline layout source correction (2026-11-02): rechecked Pillow 12.2 `ImageText.Text._split` and aligned Rust layout with its horizontal and top-to-bottom paths, mode-specific fontmode metrics, one-line early return, anchor defaults and validation order, alignment errors, Unicode character indexing, center/right/justify offsets, and spacing formula. Positioned lines are prepared before raster composition, which remains in exact-mode Draw owners. The Rust `draw_text_at` split into integer coordinates plus signed fractional offsets matches Pillow `int(line.x/y)` plus `math.modf(line.x/y)` in `ImageDraw.text`. `pillow-rs#brg0` output cases remain unverified until the CPU source-completion gate. Sources: [Pillow 12.2 ImageText.Text._split](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageText.py#L1724-L1940), [Pillow 12.2 ImageDraw.text](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py#L3105-L3326). Source-only; no build, formatting, lint, test, parity, or benchmark ran.
- Draw wide-integer cache-key correction (2026-11-02): Python arbitrary-size tuple integers now cross the PyO3 boundary as normalized sign plus exact big-endian magnitude, and integral JavaScript number components use the same core representation. The Draw palette cache distinguishes different wide values and preserves Python numeric equality between a wide integer and an exactly equal float. The component type and comparison helpers live in `image/pixel_access/palette_components.rs`, keeping `pixel_access.rs` below the source-size cap. Source-only; no build, formatting, lint, test, parity, or benchmark ran.
- ImageOps.expand ordering review (2026-11-02): Pillow 12.2 unpacks `_border`, computes `left + width + right` and `top + height + bottom`, resolves fill and palette color, then calls `Image.new`, which validates size synchronously. The current adapter first narrows border values to C integers, while core defers expanded-dimension validation until materialization. The exact custom-arithmetic and color-before-size ordering gap is now explicit in the `ops_expand` TODO and remains pending. No validation ran. Sources: [Pillow 12.2.0 ImageOps.expand](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L446-L473), [Pillow 12.2.0 Image._check_size and Image.new](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2909-L2955).
- Pad wide-integer source correction (2026-11-02): PyO3 now retains arbitrary-width integer tuple and list components for Pad; the P-mode resolver applies RGB(A) tuple alpha validation before byte-range conversion, while non-P tuple modes keep their previous converted fallback. TODO: preserve mixed/custom/nonnumeric tuple identity and finish attached/dirty/raw palette lookup and allocation order. No build, formatter, lint, test, parity, or benchmark ran. Sources: [Pillow 12.2.0 ImageOps.pad](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L2256-L2283), [Pillow 12.2.0 ImagePalette.getcolor](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImagePalette.py#L139-L168).
- Convert CoreOnly high-depth-to-I;16 continuation (2026-11-02): scalar F32-to-I;16 now clamps its numeric sample directly to u16 range and truncates; LA16 preserves its u16 luma; RGB16/RGBA16 compute luma from native u16 channels; RGB32F/RGBA32F compute luma from native normalized floats and map directly to u16. The shared high-depth owner is statically specialized per pixel type and creates no byte raster. These targets remain `NO_PILLOW_SUPPORT`; owner-level TODOs now cover focused extension parity across source modes and I;16 byte-order identities. Source-only; no compiler, formatter, lint, test, parity, or benchmark ran.
- Convert One/L source audit (2026-11-02): compared active mode contracts and dispatch with the archived SIMD Convert adapter. One normalizes its expanded byte carrier to 0/255 and exposes exactly the four I;16 target vector routes; L exposes the archived native layout, color-space, and scalar target routes plus the four I;16 routes. SSE2, AArch64 NEON, and Wasm SIMD128 declarations match the compiled modules and executor arms; One's architecture set is limited to I;16, while L palette and LAB targets use the declared wrapper compositions. L identity stays on serial Default, and vector paths preserve the archived empty-path receipts. No behavior discrepancy was found. Parity and receipt verification remain pending; no build, formatter, lint, test, parity, or benchmark ran.
- Convert I;16-source architecture audit (2026-11-02): compared the archived native-u16 vector converter with the four exact I;16 source owners. The seven archived SIMD targets (L, LA, RGB, RGBA, CMYK, I, F) map to the declared native Mode targets; each source mode exposes Default plus the same SSE2, AArch64 NEON, and Wasm SIMD128 routes, and all 21 architecture-target entries match compiled executor arms. The shared helper preserves clipping, LA/RGB/RGBA/CMYK packing, exact u16-to-I32/F32 values, and checked source/output shape construction; CoreOnly P/PA remain on their separate direct Default palette path. No source discrepancy found. Exact-mode output parity, CoreOnly I;16 aliases, and requested/selected/fallback receipts remain pending under the existing Convert owner TODOs; no build, formatter, lint, test, parity, or benchmark ran.
- Convert LA-source audit (2026-11-02): compared the exact LA support contract and 11 declared vector targets with the archived alpha, layout, color-space, and typed-lane routes. L uses the luma byte and discards alpha; RGB repeats luma; RGBA preserves alpha; RGBX preserves it in the padding lane; CMYK, YCbCr, HSV, I, and F retain their archived formulas. LA identity selects Default, LA→La has the nonempty vector guard, and One/I;16/P/PA/LAB remain on Default or their declared palette/LAB compositions. Empty inputs retain scalar-control receipts except the archived LA→RGBX swizzle route, which retains its zero-block vector receipt. No source discrepancy found. Exact-mode parity, alpha edge values, empty receipts, nested composition receipts, and architecture fallback remain pending; no build, formatter, lint, test, parity, or benchmark ran.
- Convert RGB-source audit (2026-11-02): reconciled all 20 RGB8 targets and 38 architecture guard entries with the archived luma, alpha, byte-layout, color-space, I/F, palette, binary, and LAB routes. The exact luma coefficients, opaque LA/La/RGBA/RGBa/RGBX expansions, CMYK inverse, YCbCr tables, HSV tie-breaking, RGB→I and RGB→F arithmetic, rounded-L-before-I;16 widening, and LAB scalar-control composition match their source paths. Five L-target architecture choices (AVX2, SSSE3, SSE2, NEON, Wasm SIMD128) are isolated from the SSE2/NEON/Wasm routes for the other declared targets; same-mode RGB is Default, and palette/binary/I;16/LAB routes stay on their declared Default or wrapper paths. No source discrepancy found. Exact-mode parity, target/build availability, output receipts, palette metadata, and fallback outcomes remain pending under the Convert owner TODO; no build, formatter, lint, test, parity, or benchmark ran.
- Convert RGBA-source audit (2026-11-02): compared all 36 architecture entries and the Default, SSE2, SSSE3, AVX2, NEON, Wasm SIMD128, and palette routes with the archived serial and SIMD converters. The exact Q16 luma formula, alpha preservation, RGBX opaque padding, rounded premultiplication, CMYK inverse, YCbCr/HSV transforms, I/F scalar formulas, and native RGBA-to-RGB layout path match their source behavior. AVX2 is limited to L; SSSE3 adds RGB and L; NEON retains its dedicated interleaved RGBA-to-RGB kernel with the archived allocation fallback; SSE2/NEON/Wasm handle the other declared vector targets. Same-mode RGBA selects Default; I;16 identities, LAB, P, and PA remain on their declared CoreOnly or composition routes. No source discrepancy found. Exact-mode parity, alpha edge values, empty receipts, architecture availability/fallback, palette metadata, and target-specific receipts remain pending under the Convert owner TODO; no build, formatter, lint, test, parity, or benchmark ran.
- Convert RGBa-source audit (2026-11-02): compared all 35 architecture declarations and the Default/SSE2/SSSE3/AVX2/NEON/Wasm/palette paths with the archived premultiplied-alpha and color converters. RGB and RGBA/RGBX restore straight samples with floor division, alpha zero preserves stored color, over-alpha samples clip, and L uses the dedicated unpremultiply-then-luma kernel; LA retains stored premultiplied-channel luma and source alpha. La's native extension restores RGB before luma and writes opaque alpha. CMYK, YCbCr, HSV, I, and F continue to use the archived raw RGB lanes. RGBa→P/PA quantizes the stored RGB lanes; PA's per-pixel alpha is opaque, matching the archived topalette route. AVX2/SSSE3/SSE2/NEON/Wasm L routes and the nonempty guards match their executor arms; same-mode RGBa is Default. NO_PILLOW_SUPPORT targets remain explicitly scoped as CoreOnly. No source discrepancy found. Exact-mode parity, alpha edges, empty receipts, architecture availability/fallback, palette metadata, and target-specific receipts remain pending; no build, formatter, lint, test, parity, or benchmark ran.
- Convert RGBX-source audit (2026-11-02): compared all 36 architecture entries and Default/SSE2/SSSE3/AVX2/NEON/Wasm/palette routes with the archived RGBX byte-layout converters. Every conversion ignores the fourth padding byte; RGB drops it, RGBA and LA synthesize opaque alpha, CMYK/YCbCr/HSV/I/F consume only RGB, and P/PA use the RGB lanes with opaque PA alpha. Luma uses the exact RGB Q16 formula. AVX2/SSSE3/SSE2/NEON/Wasm L routes, SSSE3/SSE2/NEON/Wasm RGB routes, and broad SSE2/NEON/Wasm routes match their execution arms. RGBX→RGBA, La, RGBa, LAB, and I;16 identities remain explicitly marked CoreOnly where applicable; same-mode RGBX is Default. No source discrepancy found. Exact-mode parity, target/build availability, empty receipts, palette metadata, and fallback outcomes remain pending under Convert's existing TODOs; no build, formatter, lint, test, parity, or benchmark ran.
- Convert CMYK-to-RGBX coverage correction (2026-11-02): the exact Pillow converter table includes CMYK→RGBX through the same four-byte CMYK-to-RGB kernel as RGBA, with an opaque fourth byte. Added RGBX to the CMYK target contract and Default executor, preserving the distinct output mode identity and keeping this route on Default because the archived SIMD specialization is RGB-only. Source correction only; target parity and routing receipts remain pending, and no build, formatter, lint, test, parity, or benchmark ran.
- Convert CMYK-source audit (2026-11-02): reconciled the declared target set, CoreOnly scope, binary/palette/LAB compositions, and four RGB architecture routes with the archived CMYK converter. The Q8 MULDIV255 CMYK-to-RGB arithmetic matches in Default, SSE2/Wasm, SSSE3, and NEON paths; RGBA and RGBX use the same RGB triplet with opaque fourth byte, and HSV composes the same CMYK expansion with the archived HSV transform. L and binary keep their distinct rounded-luma and truncated-threshold paths. The audit found and corrected the missing Pillow-supported CMYK→RGBX target; the remaining targets are either direct Default routes or explicitly CoreOnly/compositions. Exact-mode parity, binary dither cases, architecture fallback/availability, empty receipts, and target receipts remain pending; no build, formatter, lint, test, parity, or benchmark ran.
- Convert YCbCr-source audit (2026-11-02): the exact Pillow-supported targets L, LA, and RGB map to native Y-byte copy, Y plus opaque alpha, and the fixed-point YCbCr→RGB converter. Identity preserves the native YCbCr triplet. The other declared outputs are direct Default extensions or the declared binary, palette, and LAB compositions; no architecture specialization is claimed for this source mode. CoreOnly scope matches the archived converter table, and every admitted target reaches an implementation or composition. No source discrepancy found. Exact-mode parity, empty receipts, nonstandard-output parity, palette metadata, and target/rejection coverage remain pending; no build, formatter, lint, test, parity, or benchmark ran.
- Convert HSV-source audit (2026-11-02): the only Pillow-defined target, RGB, uses the archived HSV float-sector conversion with positive nearest-integer rounding. HSV identity preserves H/S/V bytes. All other admitted targets are explicitly CoreOnly and convert through RGB, the dedicated binary route, or palette/LAB composition; the support declaration claims no architecture specialization and each target reaches an implementation. No source discrepancy found. Exact-mode parity, HSV boundary values, empty receipts, extension outputs, palette metadata, and target/rejection coverage remain pending; no build, formatter, lint, test, parity, or benchmark ran.
- Convert I/F scalar-source audit (2026-11-02): the I strategy's Pillow scope matches the 12.2 converter table: L/F/RGB/RGBA/RGBX/HSV, I;16/I;16L/I;16B, plus same-mode identity; I;16N and the other declared pairs remain CoreOnly. Its Default routes preserve signed little-endian samples for clamp/truncate, numeric I→F casting, RGB-family broadcast/opaque padding, HSV `(0,0,V)`, and native u16 output byte order. The F strategy's Pillow scope is L/I and identity; declared I;16 byte-order outputs and other routes stay CoreOnly, matching 12.2. The archived SIMD byte-layout admission does not include I/F scalar source carriers, so their active contracts correctly select Default only. F→I uses the direct scalar cast; TODO beside the code records NaN/out-of-i32-range uncertainty because Pillow 12.2's C cast has no portable result there. Source references: [Pillow 12.2.0 Convert.c I routes](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L1402-L1410), [I;16 routes](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L1463-L1486). Exact-mode parity, F conversion edge cases, CoreOnly targets, empty receipts, palette metadata, and target/rejection coverage remain pending; no build, formatter, lint, test, parity, or benchmark ran.
- Convert P/PA/LAB source audit (2026-11-02): reconciled the native P and PA strategy contracts with Pillow 12.2's palette converters (`p2bit`/`pa2bit`, `p2l`/`pa2l`, `p2pa`, `p2la`/`pa2la`, `p2i`/`pa2i`, and `p2f`/`pa2f`) and their RGB-derived CMYK/YCbCr/HSV routes. P resolves palette RGB and palette-entry alpha at the Image wrapper boundary; PA resolves RGB by index and retains its own per-pixel alpha for LA/RGBA. P→PA consumes wrapper-resolved transparency/alpha, while PA→P retains the source palette metadata. RGBX is declared as a wrapper composition for P/PA. P→LAB and PA→LAB are CoreOnly compositions; PA→I;16 byte-order targets are CoreOnly native u16 outputs. LAB's same-mode identity is Pillow-supported, while its non-identity conversions remain CoreOnly. Source references: [Pillow 12.2.0 palette converter table](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L3713-L3917), [Pillow modes](https://pillow.readthedocs.io/en/12.2.0/handbook/concepts.html#_modes). The PA strategy now writes La/RGBa directly from palette RGB and per-pixel alpha, with no intermediate RGB raster; these CoreOnly routes remain parity-unverified. The lazy cross-mode palette metadata terminal remains pending under the existing Convert TODO. Exact-mode parity, palette transparency forms, empty receipts, wrapper metadata, and composed-route receipts remain unverified; no build, formatter, lint, test, parity, or benchmark ran.
- Convert La/high-depth source audit (2026-11-02): the La strategy keeps its native two-byte carrier for identity and unpremultiplies only the luma channel for LA, preserving alpha with floor division, alpha-zero passthrough, and clipping; its SSE2/NEON/Wasm guards and shared vector block path match the archived alpha converter, with Default fallback for empty input. LA16/RGB16/RGBA16/RGB32F/RGBA32F each expose an explicit CoreOnly target contract and Default-only executor. Their direct mode owners keep source pixels typed; the shared statically monomorphized helper writes each requested destination without routing through a materialized intermediate DynamicImage, and I;16 aliases read native u16 luma. Source review found and corrected LA16→RGBX selecting RGB and discarding alpha; its CoreOnly composition now selects RGBA and retains the projected LA alpha byte in X, matching the existing LA→RGBX behavior. PA and all five internal high-depth source modes now have direct Default strategies for La/RGBa; alpha-bearing PA/LA16/RGBA16/RGBA32F preserves source alpha, while RGB16/RGB32F supplies opaque alpha. CMYK/YCbCr/HSV/LAB also write directly from mode-owned samples without an intermediate RGB raster. These CoreOnly routes remain parity-unverified. Full high-depth target/float-boundary parity remains TODO beside the helper. Exact-mode parity, alpha and float limits, palette outputs, architecture fallback receipts, and composed metadata remain unverified; no build, formatter, lint, test, parity, or benchmark ran.

- Pad mixed-numeric tuple source correction (2026-11-02): Pillow 12.2's `Image.new("P")` takes its temporary RGB-palette route only for all-integer tuples of length three or four. Mixed int/float tuples continue to the single-band core fill and raise its `color must be int or single-element tuple` TypeError, so the Pad adapter retains their ordinary mode conversion instead of treating them as palette colors. Valid all-integer P tuples retain the temporary index-zero fill; four-component alpha is checked before RGB byte conversion. This differs from `ImageOps.expand`, whose existing palette route calls `ImagePalette.getcolor` directly and has separate tuple coercion. Malformed wide-integer tuples, non-P wide-integer coercion, custom/nonnumeric tuple objects, and dirty/raw palette ordering remain TODO; parity and architecture receipts are pending. No build, formatter, lint, test, parity, or benchmark ran. Sources: [Pillow 12.2.0 Image.new](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2957-L2968), [ImagePalette.getcolor](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImagePalette.py#L131-L169), and [ImageOps.pad](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L305-L345).
- ImageOps geometry source-size split (2026-11-02): moved the Expand entry points and their P palette resolver into `pillow-rs/src/ops/imageops/geometry/expand.rs`, keeping the existing `geometry` exports unchanged. The main geometry module is 855 lines and the new file is 157 lines; the extracted functions remain under the 100-line body limit. This is a behavior-preserving source-layout change only. No build, formatter, lint, test, parity, or benchmark ran.
- ImageOps.pad host coercion ordering (2026-11-02): PyO3 now checks the prepared contain result before converting optional color or centering values. When contain already fills the target dimensions, the binding sends neutral values to core and never invokes those ignored host coercions, matching Pillow's direct resized-image return. When padding is needed, color conversion still precedes centering conversion and core color resolution precedes centering indexing. TODO: verify user-defined coercion side effects that mutate the source after the lazy contain plan, malformed/custom color behavior, and dirty/raw palette failure ordering. Source-only; no build, formatter, lint, test, parity, or benchmark ran.
- Pad centering source continuation (2026-11-02): after the fill-before-centering fix, PyO3 now indexes only `centering[0]` for width padding or `centering[1]` for height padding. The selected coordinate is still converted directly to `f64`, while Pillow applies Python comparison, multiplication, and `round` semantics; a source TODO records that difference. The binding clones the source before these host hooks; lazy/COW parity for host-triggered pixel or palette mutation remains TODO. Runtime verification remains gated; no checks ran.
- Header-mode carrier retagging (2026-11-02): Pillow's decode adapter now applies the cached header `Mode` to a decoded DynamicImage when only the semantic identity differs and its sample carrier is compatible: YCbCr/LAB/HSV retain the RGB triplet carrier, RGBX retains the RGBA carrier, PA retains the LA carrier, and I;16L/B/N retain the normalized u16 carrier. The existing premultiplied LA/RGBA relabels remain at the same boundary. No pixel conversion was added. TIFF associated-alpha transfer selection is still an upstream TODO; header-versus-decoded identity and deferred decode parity remain pending. No build, formatting, lint, test, parity, or benchmark ran.

- Thumbnail dirty-palette source follow-up (2026-11-02): for every public thumbnail call, the facade asks core whether geometry reaches a different output size before coercing `resample`; requests that do not reach resize return without decoding known-size lazy sources or parsing the filter. Resize-reaching cases normalize once, then dirty P/PA wrappers commit the Python-owned palette before executing thumbnail. Core preflight uses the known-size query and mirrors geometry/no-op/same-size checks. TODO: verify no-op, geometry-error, invalid-filter, dirty-palette mutation, and resize output cases in CPU parity. Source-only; no build, formatter, lint, test, parity, or benchmark ran.

- Paste dirty-palette source audit (2026-11-02): Pillow 12.2 normalizes box syntax before source handling, loads/converts an Image source (or resolves a color string), realizes destination mutability, then loads the optional mask before pixel paste. The current facade delegates all arguments through one PyO3 call and does not realize Python-owned source, destination, or mask palettes. A staged input plan is needed to preserve that order; eagerly syncing the destination before the call would alter source/color/box error ordering. TODO remains; no behavior changed and no checks ran. Source: [Pillow 12.2.0 Image.paste](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L1692-L1773).

- Thumbnail reducing-gap contract (2026-11-02): source inspection confirms Pillow 12.2 defaults `Image.thumbnail(..., reducing_gap=2.0)` and forwards it to `resize`; the current Python facade discards the argument and the Rust Thumbnail operation retains its archived fixed reduction policy. Added an explicit `pillow-rs#3ptb` TODO. Implementing this needs a prepared operation parameter and strategy/fusion review; no algorithm change was made in this source-order correction. No checks ran. Source: [Pillow 12.2.0 Image.thumbnail](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2592-L2671).

- Thumbnail load-order correction (2026-11-02): Pillow 12.2 uses dimensions to decide no-op and output geometry before `resize()` loads pixels. Core `Image::thumbnail` and the Python binding preflight now use `Image::size()` and defer materialization until geometry and filter validation succeed; early no-op, same-size, geometry-error, and invalid-filter calls avoid decoding known-size lazy sources. This corrects the earlier assumption that thumbnail always materializes at entry. Source-only; no build, formatter, lint, test, parity, or benchmark ran. Source: [Pillow 12.2.0 Image.thumbnail and Image.resize](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2134-L2237), [Image.thumbnail](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2592-L2671).

- Thumbnail reducing-gap strategy follow-up (2026-11-02): the Python facade now preserves the Pillow default `2.0`, validates after filter normalization and before dirty-palette realization, and forwards explicit values including `None`. Rust carries the gap through the prepared Thumbnail descriptor and exact-mode CPU support facts; CPU computes mode-specific reduction factors before composing the existing Reduce/Resize strategies. GPU only expands the default `2.0` reduction and sends custom gaps to exact host control; LA/RGBA skip reducing-gap while premultiplied La/RGBa follow their exact identities. Source-only; mode × architecture, custom-gap, palette ordering, and GPU routing parity remain pending. No build, formatter, lint, test, parity, or benchmark ran. Source: [Pillow 12.2.0 Image.thumbnail and Image.resize](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2134-L2237), [Image.thumbnail](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2592-L2671).

- Paste staged wrapper follow-up (2026-11-02): the Python facade now validates box syntax first, loads/converts an Image source or resolves a color string, realizes destination mutability, then loads the effective mask before final Paste admission. Raw numeric source coercion is deferred until the final core call after destination and mask loads. Box preparation rejects default/two-coordinate boxes before source resolution if neither source nor effective mask is an image. Opaque Rust box/source plans preserve converted source state across those wrapper boundaries; the final call supplies the loaded box-as-mask image rather than reusing the pre-load image clone held for box classification. Source extraction still loses tuple/list provenance and arbitrary-width or mixed numeric components at `Vec<i64>` extraction, and a source TODO records the exact-mode coercion gap. Other TODOs cover custom/effectful conversions and reentrant palette hooks between stages, Python two-point endpoint arithmetic and custom `__index__` timing, the size needed by a falsy Image subclass used as the abbreviated mask, arbitrary-width Python box coordinates, and destination mutation/error order. The CPU mode/architecture strategies remain unchanged. Source-only; no build, formatter, lint, test, parity, or benchmark ran. Source: [Pillow 12.2.0 Image.paste](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L1692-L1773).
- I;16 Paste scalar source correction (2026-11-02): the Pillow 12.2 `getink` source parses the scalar as a signed long long, then stores its low two bytes in the special 16-bit sample; it does not duplicate the low byte. `paste_scalar_source` now retains the raw integer through exact-mode resolution, and the I;16 color resolver takes its low 16 bits before creating the native u16 source. Both Python staged inputs and the typed Rust scalar route use this numeric-sample interpretation. Source-only; I;16/I;16L/I;16B/I;16N scalar parity and byte-order behavior remain in the frozen CPU matrix. No build, formatter, lint, test, parity, or benchmark ran. Source: [Pillow 12.2.0 `_imaging.c::getink`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L534-L645).
- I-mode Paste scalar narrowing (2026-11-02): the same Pillow getink routine converts its signed long-long ink into an INT32 sample by storing the low four bytes. The Rust I-mode color resolver now preserves that two's-complement narrowing instead of rejecting values outside i32 while they remain inside Pillow's signed-long-long range. Arbitrary-precision Python integers outside i64 still need source-aware overflow translation; CPU parity remains pending. No build, formatter, lint, test, parity, or benchmark ran. Source: [Pillow 12.2.0 `_imaging.c::getink`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L534-L645).
- Paste typed color input continuation (2026-11-02): built-in Python tuple/list provenance is retained through the host-neutral Paste input; list colors are rejected before inspecting elements. Mode-aware source classification now follows Pillow's `F` float conversion path and avoids treating arbitrary `__index__` objects as scalar integers for byte modes. Built-in tuple components retain integer, wide-integer, float, and invalid-type categories; one-band and multi-band non-tuple errors are separated at the core input owner. Wide integer preparation no longer calls overridable Python integer methods or allocates an unused magnitude buffer. Remaining TODOs: exact custom `__index__` tuple coercion after arity checks, effectful conversion/reentrant hooks, singleton tuple error details, Python endpoint arithmetic/falsy-mask size, and arbitrary-width coordinate overflow. Source-derived from [Pillow 12.2.0 `_imaging.c::getink`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L3092-L3309); no build, formatter, checkstyle, lint, test, parity, or benchmark ran.
- Paste tuple index protocol continuation (2026-11-02): correctly shaped byte-mode tuples now invoke Python's index protocol in field order and stop at the first conversion failure; C long and C int boundaries remain distinct. Invalid tuple arities are rejected without inspecting components. Singleton tuple diagnostics and coordinate-box coercion remain TODO. Source-only; no build, formatter, checkstyle, lint, test, parity, or benchmark ran.

- Paste box coordinate continuation (2026-11-02): tuple/list arity is checked before reading coordinate components. Valid boxes use Python `operator.index` for each component and preserve PyLong/C-long overflow instead of collapsing conversion failures into a misleading wrong-length error. Arbitrary iterable normalization, list-subclass iteration hooks, deferred custom coercion, two-coordinate endpoint arithmetic, and stage ordering remain TODO. Source-only; no build, formatter, checkstyle, lint, test, parity, or benchmark ran.

- Paste box iterable normalization (2026-11-02): the Python facade now snapshots every non-tuple, non-Image box with `tuple(box)` before box arity checks, matching Pillow wrapper ordering and preserving iterable exceptions before source work. Coordinate `__index__` remains eager in the PyO3 boundary; custom coercion timing, two-point endpoint arithmetic, and reentrant image mutation remain TODO. Source-only; no build, formatter, checkstyle, lint, test, parity, or benchmark ran.

- Paste coordinate staging (2026-11-02): the facade/PyO3 plan validates tuple/list arity before retaining raw coordinate objects; the staged apply boundary resolves the source then applies `operator.index` and C-long conversion after source, destination, and mask loads. The direct low-level PyO3 `paste` method retains eager conversion. Python two-coordinate endpoint addition, falsy abbreviated-mask size, reentrant mutation ordering, and singleton tuple diagnostics remain TODO. Source-only; no build, formatter, checkstyle, lint, test, parity, or benchmark ran.

- Paste box order correction (2026-11-02): Pillow 12.2 does not tuple-normalize arbitrary boxes. It handles an Image box first, replaces `None` with `(0, 0)`, reads `len(box)`, and for two coordinates reads `box[0:2]`, selects source or mask size, and adds endpoints before source resolution/load. The facade now mirrors those steps and passes the observed length into the staged box plan to avoid calling `len` a second time. Custom sequence-hook/reentrant mutation parity and the direct low-level PyO3 paste path remain TODO. Source: [Pillow 12.2.0 Image.paste](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L1692-L1773). Source-only; no build, formatter, checkstyle, lint, test, parity, or benchmark ran.

- Paste endpoint and coordinate-hook ordering (2026-11-02): Python `Image.paste` now performs Pillow's Image-as-mask check through staged preparation, `None` defaulting, `len`, source/mask size selection, and two-coordinate `box += (...)` before source work. Tuple/list coordinate objects remain held until final apply; arbitrary sequence `__getitem__` is interleaved with `operator.index` in index order after source/destination/mask loads. The known sequence length is passed from the facade to avoid an extra `len` call. TODO remains for unusual subclass mutation/length/add behavior, falsy abbreviated masks, reentrant image mutations, singleton tuple coercion, and the separate low-level PyO3 paste method. Source: [Pillow 12.2.0 Image.paste](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L1692-L1773). Source-only; no build, formatter, checkstyle, lint, test, parity, or benchmark ran.

- Paste adapter consolidation (2026-11-02): repository-wide source search found no callers of the low-level PyO3 `paste` method; the Pillow facade exclusively uses its staged prepare/apply methods. Removed the eager bypass and its parser, leaving the staged route as the single Python binding path. `Image::paste_with_input` remains available for the JavaScript adapter. No runtime validation ran.

- Paste abbreviated Image-mask normalization (2026-11-02): the Python facade now converts `paste(source, image_mask)` into the ordinary mask flow before box expansion. A falsy Image subclass still supplies its size to two-coordinate endpoint arithmetic, while final truthiness controls whether it is loaded/applied. The prepared box no longer carries a pre-load image-mask clone, and the PyO3 apply method no longer accepts a separate loaded-box-mask parameter. Source-only; no build, formatter, checkstyle, lint, test, parity, or benchmark ran.

- Paste post-unwrapping singleton diagnostics and final parse order (2026-11-02): Pillow 12.2's `getink` unwraps one-element tuples; LA/La/PA then call `PyArg_ParseTuple("L|i", ...)` with the unwrapped object, so non-integer singleton items retain CPython's exact `SystemError` and message. Final staged Paste now parses box shape/coordinates after source, destination, and mask loading but before deferred numeric source coercion, matching `_paste` argument parsing before `getink`. Generic box sequences are length-checked and snapshotted before coordinate `operator.index` coercion. Remaining TODOs are custom sequence/iterator mutation and reentrant image mutations. Sources: [Pillow 12.2.0 `_imaging.c::getink`/`_paste`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L534-L649), [CPython 3.13 `PyArg_ParseTuple`](https://github.com/python/cpython/blob/v3.13.0/Python/getargs.c#L84-L93), [CPython 3.13 tuple validation](https://github.com/python/cpython/blob/v3.13.0/Python/getargs.c#L348-L370). Source-only; no build, formatter, checkstyle, lint, test, parity, or benchmark ran.

- CMYK Text integer-ink correction (2026-11-02): Pillow Draw resolves integer multi-band colors to all four packed little-endian bytes in `_getink`/`getink`. The mode-owned CMYK Text helper already receives that resolved four-channel ink, but incorrectly replaced its upper three channels with zero when the source input was an integer. It now blends the resolved CMYK channels directly, matching Pillow's ink resolution and the archived raw-carrier behavior. The redundant integer-origin flag was removed from the prepared raster input; it remains available at strategy selection, where support assessment consumes it. Sources: [Pillow 12.2.0 `ImageDraw._getink`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py#L135-L158), [Pillow 12.2.0 `_imaging.c::getink`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L534-L645). The tracked `pillow-rs#brg0` CMYK Text and MultilineText fixtures use `fill=None`, so this explicit-integer correction is not claimed to address those failures. Verify explicit packed integer colors separately; diagnose the default-fill CMYK mismatches independently during source review and focused CPU parity. No build, formatter, checkstyle, lint, test, parity, or benchmark ran.
- Draw packed integer narrowing correction (2026-11-02): Pillow 12.2 parses integer inks as signed `long long`; multi-band UINT8 inks take the low four bytes, and I stores the low 32 bits. The core resolver previously rejected values outside i32 even when they fit Pillow's signed-long-long input. It now preserves those low bytes for RGB/RGBA/CMYK-family inks and the low word for I. F converts through f64 before narrowing to f32, matching PyFloat_AsDouble. Values outside signed long long still need exact host-overflow parity review. Source: [Pillow 12.2.0 `_imaging.c::getink`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L3105-L3138), [multi-band UINT8 packing](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L3193-L3264), [I storage](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L3269-L3277), [F conversion](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L3279-L3295). Packed boundary parity remains pending the CPU source gate; no build, formatter, checkstyle, lint, test, parity, or benchmark ran.

- CMYK default Text parity triage (2026-11-02): the frozen `text.mode.cmyk` and `multiline_text.mode.cmyk` calls omit `fill`, so they use Pillow `ImageDraw.__init__` default `draw_ink(-1)`. For CMYK, `_imaging.c::getink` extracts four 255 bytes; `Draw::text_color_with_input(None)` returns the same tuple, and the exact-mode CMYK owner blends each channel with the same mask coverage and `pil_blend` formula as the archived raw carrier path. This source trace has not found the historical mismatch cause; default-fill CMYK output remains a source-owner TODO for the gated focused parity run. Sources: [Pillow 12.2.0 `ImageDraw.__init__` and `_getink`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py#L63-L158), [`_imaging.c::getink`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L3105-L3264). No build, formatter, checkstyle, lint, test, parity, or benchmark ran.

- Draw native-mode ink and Text audit (2026-11-02): `_imaging.c::getink` packs multi-band integer colors into four bytes, including two-band alpha at byte 3. The resolver now preserves that mapping for LA/La, maps LAB from the three color bytes, and maps RGBa/RGBX as four-byte modes. Pillow `fill_mask_L` treats LA/La/RGBa/RGBA/PA as alpha-bearing: color coverage is full only when destination alpha is zero, while alpha itself blends with mask coverage. The native LA/La helper, RGBA/RGBa owners, and RGBX owner now follow those per-mode rules; RGBX blends all four stored bytes. Default text inks for LAB, RGBX, and La now reflect `draw_ink(-1)`. Singleton tuples now unwrap into the scalar ink path before mode-specific resolution, including packed multi-band values and one-element F values; non-singleton single-band tuples receive their Pillow mode error. Sources: [`_imaging.c::getink`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L534-L645), [`ImageDraw.text` mask route](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py#L557-L611), [`Paste.c::fill_mask_L`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Paste.c#L418-L472). Exact mode output, packed scalar boundaries, tuple-component C-int overflow, and wide-integer parity remain pending under `pillow-rs#3ptb` and `pillow-rs#brg0`; no build, formatter, checkstyle, lint, test, parity, or benchmark ran.
- Draw stroked-RGBA source correction (2026-11-02): Pillow’s `ImageDraw.text` uses an L coverage mask for ordinary text and uses an RGBA mask plus `paste` only for embedded-color text. When `stroke_fill` differs from `fill`, each pass still uses its own mask semantics; pass ordering must not change the coverage algorithm. Removed `blend_rgba_overlay` from the composed input and made the RGBA owner choose between alpha-aware L-mask coverage and RGBA-mask paste coverage from its compile-time mask layout. This aligns ordinary stroked text on transparent pixels while preserving embedded-color compositing. Sources: [Pillow 12.2.0 `ImageDraw.text`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py#L503-L621), [`Paste.c::fill_mask_L`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Paste.c#L418-L472). Source-only; no build, formatter, checkstyle, lint, test, parity, or benchmark ran.
- Histogram masked two-band source review (2026-11-03): Pillow 12.2 `Histo.c` reads physical lane 1 into the masked second result bank and lane 3 for unmasked two-band input. LA/La/PA unpack and ordinary two-component inks initialize the omitted lanes by repeating the first sample. The maintained typed Histogram owner reads retained lane 1 when available and falls back to that canonical sample when provenance is absent. Source review confirms new/frombytes/decode construction, packed PutData, and same-pixel-size mode-mismatched Paste retain independent lane values in `HiddenPixelLanes`; this repository has no Arrow image-import API, so `ImagingNewArrow` is not a reachable producer. The remaining cross-operation risk is a producer that drops hidden-lane provenance before Histogram; track it through the shared hidden-lane producer audit. Sources: [Histo.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Histo.c#L721-L830), [Unpack.c mode dispatch](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Unpack.c#L1493-L1511), [`unpackLA`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Unpack.c#L2924-L2957), [`_imaging.c::getink`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L534-L645), [Paste.c raw pixelsize path](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Paste.c#L1982-L2017). Source-only; all Histogram parity remains gated until CPU source completion.
- Histogram logical-pixel bound correction (2026-11-03): the Default Histogram scans now iterate typed `ImageBuffer::pixels()` rather than every complete sample group in the backing `Vec`. Pillow Histo.c traverses the image dimensions; trailing carrier samples are not pixels. This also preserves the archived row-based bounds for byte-mode scans while keeping CoreOnly u16/f32 and scalar I/F scans native, allocation-free, and bounded to width × height. No mode conversion was added. Source-only; no build, formatter, checkstyle, lint, test, parity, or benchmark ran. Source: [Histo.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Histo.c#L721-L830).
- GetExtremaBytes logical-pixel bound correction (2026-11-03): all 25 exact-mode byte-valued extrema owners now pass a checked width × height sample prefix to the native reducer. Trailing samples in oversized `ImageBuffer` containers no longer affect byte extrema. The helper adds no pixel conversion or staging allocation. Source-only; no build, formatter, checkstyle, lint, test, parity, or benchmark ran.
- CoreOnly Histogram support annotation (2026-11-02): native multiband u16/f32 scans and RGB32F’s synthetic opaque fourth band are documented as `NO_PILLOW_SUPPORT`, since these exact Rust modes have no Pillow mode identity. Histogram parity for the Rust-only extension remains pending the CPU parity gate.
- Thumbnail geometry plan staging (2026-11-02): `Image::prepare_thumbnail` now returns opaque target geometry and `thumbnail_with_plan` applies it after host coercion. The Python facade floors/unpacks the original size, performs `reducing_gap` multiplication/int conversion and calls `draft` before its post-draft size check, then routes a returned draft box through the existing boxed Resize path. Filter parsing and gap float conversion use module helpers outside a `PyImage` borrow, so user callbacks can call back into the wrapper; the already selected target dimensions are not recomputed against a changed image size. Removed the unused low-level PyO3 `thumbnail` bypass so the Python facade has one staged binding route. Remaining TODO: source-review callback-driven mode/storage changes and `getdata` identity when a filter/gap callback resizes the image to the selected target. Sources: [Pillow 12.2.0 `Image.thumbnail`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2592-L2671), [`Image.resize`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2134-L2237). Source-only; no build, formatter, checkstyle, lint, test, parity, or benchmark ran.

- Paste deferred-box parser correction (2026-11-03): Pillow's `_imaging.c::_paste` parses the final box with `PyArg_ParseTuple(..., "O(iiii)|O!", ...)`; CPython 3.13 `converttuple` checks `PySequence_Check`, rejects bytes, calls `PySequence_Size` once, then reads each coordinate using ordered `PySequence_GetItem` calls. The binding now retains the original post-wrapper box object—including tuple subclasses and objects returned by two-coordinate `__iadd__`—and performs those same indexed reads after source, destination, and active-mask loads. It no longer snapshots generic boxes through `tuple(value)`, rejects Unicode/bytearray sequences prematurely, or treats an augmented-add result that happens to be an Image as Pillow's pre-wrapper Image-as-mask shorthand. Failed indexed reads clear the original exception and report the parser's `argument 2, item N is not retrievable` error. Coordinate conversion now uses PyO3's `c_long` extractor, which calls `PyLong_AsLong` and preserves Pillow's `__index__`, type-error, and long-overflow contract; it immediately checks C-int bounds before fetching the next sequence item, preserving first-failure order. Remaining Paste TODO is limited to reentrant image/palette mutation parity. Sources: [Pillow 12.2.0 `_imaging.c::_paste`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L1369-L1405), [CPython 3.13 `converttuple`](https://github.com/python/cpython/blob/v3.13.0/Python/getargs.c#L431-L497). Source-only; no build, formatter, checkstyle, lint, test, parity, or benchmark ran.

- Thumbnail reentrant callback source closure (2026-11-03): the geometry plan captures only Pillow's pre-callback target dimensions. `draft`, filter conversion, and reducing-gap conversion may affect the wrapper before execution, so mode and storage routing must read the current `DynamicImage`. If filter/gap coercion changes the wrapper to the selected target size after Pillow has entered the resize branch, the facade now performs and assigns the current mode's identity resize; this preserves Pillow's core replacement and keeps prior `getdata` sequences attached to their earlier storage. The existing pre-filter return remains when `draft` itself reaches the target. Source: [Pillow 12.2.0 `Image.thumbnail`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2634-L2671). Runtime callback/mode/storage and `getdata` identity parity remains pending; no build, formatter, lint, test, parity, or benchmark ran.
- Paste reentrant callback source review (2026-11-03): the facade loads the source, invokes virtual `convert(self.mode)` when Pillow would, and captures the resolved raster in `PasteSourcePlan` before destination and active-mask loading. The resolved-source plan queues that raster without a second mode conversion; the exact-base-image path keeps conversion in the typed Rust strategy. Source and palette mutation cases remain CPU parity work; the subclass override itself has a version-pinned TODO below.

- ImageOps resize-filter preflight (2026-11-03): `contain`, `cover`, and `fit` now preflight core geometry before strict Python filter coercion; Fit also resolves centering before aspect validation. Rounded-zero output dimensions remain validated after filter parsing at Resize. Follow-up: the Python facade now captures contain/cover dimensions or fit's crop box, parses the filter, then calls `Image.resize` on the wrapper's current image. Reentrant filter callbacks therefore retain the saved geometry while exact-mode Resize routing reads the current `DynamicImage`; full-box identity requests copy the current image after filter validation. The obsolete one-shot PyO3 contain/cover/fit operations were removed in favor of geometry and strict-filter helpers. Runtime callback, mode, palette, and architecture parity remains pending. Sources: [Pillow 12.2.0 `ImageOps.contain` and `cover`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L249-L303), [`ImageOps.fit`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L475-L558), [`Image.resize`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2134-L2237). Source-only; no build, formatter, lint, test, parity, or benchmark ran.

- UnsharpMask source comparison (2026-11-03): compared the maintained Filter strategy with archived `3f9dfd403` behavior. The byte blend remains sample-wise on the native carrier, with signed truncation and clipping preserved for arbitrary percentages; the optimized 150% formula is equivalent to truncating `difference * 3 / 2`. Pillow 12.2's Gaussian blur gate admits L, LA, La, RGB, RGBA, RGBa, RGBX, and CMYK, matching the exact UnsharpMask support declaration; One, P/PA, YCbCr, LAB, HSV, scalar, and CoreOnly high-depth identities are rejected ([source](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/BoxBlur.c#L223-L252)). The architecture-specific path remains limited to RGB8, 150%, and an available declared architecture; all other admitted modes use the same-mode Default blend. GaussianBlur is prepared as a child with the same injected Backend/Architecture context, and its owned output allocation is reused for the blend. Moved CPU route resolution before materialization to restore the archived error-order boundary while keeping the materialized `DynamicImage` as the exact strategy-mode authority. Source-only; mode, parameter, fallback, receipt, and error-order parity remains pending. No validation ran.

- Histogram I/F undefined-arithmetic source audit (2026-11-03): Pillow 12.2 Histo.c computes I extrema/sample differences in signed `INT32`, whose overflow is undefined, and casts the scaled F32 result directly to C `int`, which is undefined for NaN, infinity, or values outside the C int range ([source](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Histo.c#L142-L190)). The native Rust owners retain float32 scale/truncation and define these edge cases explicitly: I uses two's-complement wrapping; F omits non-finite and out-of-i32-range bins before casting. Added `NO_PILLOW_SUPPORT` comments at the exact helpers. Full-range I and explicit-extrema F boundary cases remain frozen CPU parity TODOs; no validation ran.

- Stat I/F undefined-arithmetic source audit (2026-11-03): unmasked `Stat` uses its native scalar histogram reducer rather than allocating a Histogram child. I samples are widened to f64 before range scaling, defining full-range behavior where Pillow Histo.c signed `INT32` subtraction overflows; F samples keep Pillow's float32 scale and explicitly define the otherwise-undefined non-finite and out-of-range float-to-int casts through Rust clamp/cast behavior. Added `NO_PILLOW_SUPPORT` comments beside both affected branches. These extension boundaries remain frozen CPU parity TODOs; source-only, no validation ran.

- Rank-filter empty-image extension (2026-11-03): the CPU Filter owner keeps zero-sized outputs empty for windows of size three or larger, while Pillow 12.2's `ImagingExpand` path reads an edge pixel from the empty input. Added `NO_PILLOW_SUPPORT` beside the Filter owner as the explicit reason for the safe extension; the exact empty-window cases remain in the frozen CPU parity TODO. Source: [Pillow advisory for rank-filter empty-image ImagingExpand access](https://github.com/python-pillow/Pillow/security/advisories/GHSA-27h8-gf7j-v5hc). No build, formatter, lint, test, parity, or benchmark ran.

- Paste subclass override compatibility (2026-11-03): the user reproduced Pillow 12.3.0 with a subclass `convert()` returning its original image. RGBA→CMYK, RGB→RGBA, and L→P succeed because physical pixel sizes match; RGBA→L raises `ValueError: images do not match`. Pillow calls the virtual conversion without rechecking the returned mode, then native Paste checks dimensions and internal pixel size and copies or blends physical bytes. Treat this as undocumented compatibility, not a public promise that mode names are interchangeable. The input-only parity protocol and builder now construct an instance override that returns its source and generate those four cases plus masked LA→RGBA lane blending. The repository oracle remains pinned to Pillow 12.2.0, so generated cases verify that active profile; the user's 12.3.0 reproduction is separate evidence. Runtime parity remains pending. Do not contact maintainers or open external issues.
- Paste resolved-source capture (2026-11-03): the facade invokes virtual `convert(self.mode)` for source subclasses and overridden instance/class methods before destination/mask loads, while the exact base-image method keeps conversion in Rust's typed Paste strategy. Already-resolved sources are cloned into `PasteSourcePlan` immediately, preserving the raster graph captured before destination and active-mask callbacks. Tracing the current call chain shows that `prepare_resolved_paste_image_source` stores the already-resolved Python source and `paste_prepared_source` queues it unchanged; this route does not call `resolve_image_source`. The ordinary base-image path converts differing modes except Pillow's `LA`/`RGBA`/`RGBa`-to-`RGB` performance fast path; subclass `convert()` overrides can supply a resolved image whose actual mode differs from the request. CPU Paste now admits same-width 1-byte, 2-byte I;16, and 4-byte layouts for resolved inputs, reconstructing omitted physical lanes on compact Rust carriers; ordinary conversion still occurs before backend admission. `RGBa`-to-`RGB` follows the raw shortcut, preserving premultiplied samples rather than unpremultiplying them. I;16 byte-order, mask, and clipping parity remain TODO; PutPixel, synchronous and bulk PutData, and LA-family PutAlpha/PutAlphaData now retain their affected lanes. Draw, in-place batch writers, and mode-preserving outputs still need exact-mode lane review before downstream Paste/Histogram parity can be claimed. Reentrant image/palette mutation and unusual wrapper/core aliases also remain parity TODOs. Sources: Pillow 12.2.0 [`Image.paste`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L1756-L1764), [`Paste.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Paste.c#L270-L329), and [the 1.0b2 performance change](https://github.com/python-pillow/Pillow/blob/12.2.0/CHANGES.rst#L2845-L2849). Source-only; no build, formatter, checkstyle, lint, test, parity, or benchmark ran.
- PutData I;16 bytes-subclass source audit (2026-11-03): Pillow 12.2 obtains `n` with `PyObject_Length`, then reads `n` bytes directly from `PyBytes_AS_STRING` in its row-prefix path. A bytes subclass whose reported length exceeds its physical allocation can therefore make Pillow read out of bounds; that behavior is undefined, not a stable parity contract. The Rust I;16 writer keeps the valid physical prefix and zero-fills absent entries, marked `NO_PILLOW_SUPPORT` at the helper. Scaled byte values still use Pillow's `(int)(byte * scale + offset)` conversion, which is undefined for NaN, infinity, and values outside C `int`; that edge remains a TODO. Source: [Pillow 12.2.0 `_imaging.c::_putdata`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L1496-L1589). No build, formatter, checkstyle, lint, test, parity, or benchmark ran.
- PutData numeric I;16 undefined-cast boundary (2026-11-03): Pillow converts the scaled Python number to C `int` before clipping the low and high bytes. NaN, infinities, and values outside `int` range have no defined Pillow result; the maintained Rust float-to-i32 cast defines deterministic extension behavior and is marked `NO_PILLOW_SUPPORT` at its owner. Freeze that extension in a target-only CPU parity lane; ordinary representable values remain part of Pillow parity. Source: [Pillow 12.2.0 `_imaging.c::_putdata`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L1572-L1584). No validation ran.
- ImageOps Expand staged boundary (2026-11-03): the Python facade now follows Pillow's border unpacking and computes output width/height before fill conversion. A typed `PreparedExpandFill` preserves exact-mode conversion across dirty-palette realization; P palette lookup then uses the committed palette before size validation. The PyO3 boundary checks the computed dimensions through the operation's `ExpandedDimensions` contract, the facade forms Paste's four-coordinate box after size validation, and the binding applies ordered C-int parsing. The core derives trailing margins from this explicit size and queues the existing exact-mode CPU Expand strategy; no intermediate output raster is allocated. Expand dimension checks now allow valid empty outputs, matching `Image.new` geometry while retaining checked allocation limits. Remaining TODOs: verify size coercion, eager allocation failure timing, source-load/Paste parser ordering, and dirty/malformed palette behavior in the frozen CPU parity set. Source: [Pillow 12.2.0 ImageOps `_border` and `expand`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L30-L38), [ImageOps.expand](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L446-L473), [Image.new](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L2909-L2955). Source-only; no build, formatter, checkstyle, lint, tests, parity, or benchmark ran.
- ImageOps Expand source boundary follow-up (2026-11-03): border unpacking and size reads now follow Pillow's direct Python expressions, including two separate `image.size` reads. The binding snapshots the DynamicImage before Python fill coercion, preserving the mode argument to `_color`; the facade accesses `image.palette` after fill conversion, preserving indexed source load and dirty-palette realization, then snapshots the Image.new source before Python size comparisons. Negative dimensions are rejected before ordered C-int extraction; C-int range is bounded before exact-mode `ExpandedDimensions` validation. This closes the source-visible negative-size error-class gap and omitted P/PA palette-load boundary. The facade now constructs the expanded paste box, loads the current source, then allows ordered native coordinate parsing, matching Pillow's `Image.paste` source-load order while keeping the expanded pipeline lazy. Remaining TODOs: source mutation during palette access or size comparisons, eager `Image.new` allocation failure timing, and dirty/malformed palette parity. Source-only; no build, formatter, checkstyle, lint, test, parity, or benchmark ran.
- Direct lazy Convert to P/PA now executes its exact prepared CPU strategy and retains the resulting palette, per-entry alpha, and wrapper metadata beside the lazy pipeline result. The sidecar is published only after output-mode validation; palette access remains non-materializing, while `load`, `copy`, metadata queries, and palette-preserving follow-on pipelines consume the cached result. The uncached `verify()` path propagates a scratch sidecar through nested pipelines without publishing materialized state. Source-only review; CPU parity, architecture/fallback receipts, and palette metadata behavior remain pending. No build, formatter, checkstyle, lint, test, parity, or benchmark ran.
- RGBA GaussianBlur radius-seven source comparison (2026-11-03): compared the maintained Default and architecture-selected owners with the archived CPU/SIMD route from `3f9dfd403`. The three horizontal then three vertical passes, Pillow Q24 weights and edge replication, AVX2 eight-pixel prefix kernel, NEON row-major vertical threshold (`width >= 64 && height >= 64`), and transpose-based fallback order match by source inspection. Prepared architecture selection admits the specialized AVX2/NEON kernel only for finite nonzero sigma with derived radius seven and nonzero fractional weight; other declared architectures use the native-byte fallback, while unavailable host/build paths resolve through the prepared fallback. The large-uniform identity proof remains ahead of the specialized execution. No source discrepancy found. Focused RGBA GaussianBlur bytes/receipts and remaining Filter source review are still pending; no build, format, checkstyle, lint, test, parity, or benchmark ran.
- L rank-window source comparison (2026-11-03): compared the active exact-mode L Default and architecture strategies with the preserved CPU/SIMD implementations. Default retains clamped borders, min/max specialization, small-window order-statistic selection, and the histogram path for larger windows. The architecture owner retains 16-lane gathers, dedicated 3x3 second-minimum/median networks, vector sorting through 5x5, and byte-domain selection through 15x15; exact L materialization and the declared guard cover the archived nonempty input range. The preserved SIMD source records 9x9 rank-40 slower than serial CPU on Apple ARM64. Keep that measured performance gap as a later algorithm work item; this refactor did not change it. Edge/output and architecture-fallback parity remain pending; no validation ran.
- Native-byte Filter3x3 source comparison (2026-11-03): compared the Default convolution helper and shared architecture vector kernel with the archived CPU and SIMD implementations. Coefficient normalization, bottom-to-top kernel rows, f32 accumulation order, `offset + 0.5` bias, truncating clip, and copied border bytes match by inspection. Exact-mode leaves supply the native channel count and restore their original raster variant. RGB's contiguous byte stream preserves channel spacing, and LA's binomial helper preserves the two-channel integer rounding and border. I32's typed Filter3x3 path and remaining Filter5x5 architecture routes are outside this comparison. Byte output and architecture receipt parity remain pending; no validation ran.
- I32 Filter5x5 source comparison (2026-11-03): compared Default, AVX/FMA, and AVX2/FMA owners with the preserved CPU/SIMD implementations. Default retains ImageI32 storage, scalar f32 coefficient/row accumulation order, truncation, copied border, and bounded-binomial fast path; AVX/FMA injects its token into the same scalar ring only for the guarded exact normalized binomial request. The eight-lane AVX2 kernel preserves the archived horizontal/vertical tap order, gathered final block, and copied border. AVX2/FMA and AVX/FMA uniform-identity shortcuts record the archived `scalar-control` path; positive-size requests with no interior use the shared native-copy kernel before architecture dispatch. Finite nonzero scale, finite kernel coefficients, and exact four-byte storage remain required for AVX2/FMA; AVX/FMA remains limited to the exact normalized binomial request with zero offset. I32 Filter5x5 output and selected/fallback receipts remain TODO in `pillow-rs#3ptb`; byte-mode Filter5x5 and I32 Filter3x3 source reviews are recorded in the following entries. No build, formatter, checkstyle, lint, test, parity, or benchmark ran.
- I32 Filter3x3 source comparison (2026-11-03): compared Default and AVX2/FMA with the preserved CPU/SIMD implementations. Default retains scalar f32 row accumulation and the exact cross-smooth specialization. AVX2/FMA retains four-lane f32 arithmetic, scalar tail, and copied border; its strategy guard admits checked exact four-byte storage, finite nonzero scale, finite kernel coefficients, and nonzero dimensions. Positive-size no-interior requests use the architecture-selected native-copy path; convolution requests with an interior retain the vector kernel. Prepared uniform-image admission uses the selected Default or AVX2 proof before one-op shared execution. Output and selected/fallback receipts remain pending CPU parity in `pillow-rs#3ptb`; no build, formatter, checkstyle, lint, test, parity, or benchmark ran.
- Byte Filter5x5 source comparison (2026-11-03): compared the exact-mode Default leaves and shared architecture routes with the archived CPU/SIMD implementations. The shared vector kernel retains per-channel/eight-lane and complete-pixel layouts, Pillow row accumulation, clipping, and copied borders. Mode leaves restore exact raster variants; L retains uniform/binomial kernels, LA retains its single-outlier and binomial kernels including the NEON leaf, and RGB retains the large-uniform scan and BLUR ring. The vector admission guard checks exact byte storage, nonzero dimensions, finite nonzero scale, and finite kernel coefficients; offset retains the kernel's arithmetic contract. Positive-size requests with no 5x5 interior return through the selected architecture's native-copy path, while zero-sized or otherwise ineligible requests use same-mode Default. Byte output and architecture/fallback receipts remain TODO in `pillow-rs#3ptb`; no build, formatter, checkstyle, lint, test, parity, or benchmark ran.
- Native-byte extrema/rank source comparison (2026-11-03): compared maintained MaxFilter/MinFilter and non-L MedianFilter/RankFilter architecture strategies with archived `pool_simd` kernels and serial rank code. Extrema retain exact byte-layout checks, positive odd windows, clamped separable 16-lane passes, bounded uniform/all-zero identity copies, and exact-mode output reconstruction. Rank retains 15x15/rank bounds, 16-lane gathering, LA's interleaved 3x3 selector, RGB's 3x3 median network, sorting through 5x5, and byte-domain selection through 15x15. Restored the archived RGB gather's single clamped-coordinate walk across the three columns. L remains in its exact-mode strategy. Output and architecture/fallback receipt parity remain pending in `pillow-rs#3ptb`; the archived Apple ARM64 L 9x9 rank-40 regression remains a later algorithm-focused performance item. Source-only; no build, formatter, checkstyle, lint, test, parity, or benchmark ran.
- ModeFilter source comparison (2026-11-03): compared maintained exact-mode strategies with Pillow 12.2 [`ModeFilter.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/ModeFilter.c). The one-band byte paths preserve the clipped neighborhood, ascending-value tie rule, `count > 2` threshold, and original-center fallback. Rust even-size normalization preserves Pillow's integer-halved radius, and the P wrapper returns the same palette index mode with its palette and alpha metadata. Multiband byte strategies remain explicitly marked `NO_PILLOW_SUPPORT` Rust extensions. Mode, palette, argument-order, and selected/fallback receipt parity remain pending; no build, formatter, checkstyle, lint, test, parity, or benchmark ran.
- Native-byte blur identity receipts (2026-11-03): source comparison found maintained architecture blur's zero-radius and small-uniform branches used `to_vec()` without the archived `native-copy` route receipt or vector/tail counts. Added one checked exact-mode copy helper using the existing vector-copy kernel and routed both branches through it. RGBA radius-seven identity proof now records its proof scan and the same native-copy terminal receipt. Broader BoxBlur/BoxBlurXY/GaussianBlur mode/kernel comparison and CPU parity remain pending; no build, formatter, checkstyle, lint, test, parity, or benchmark ran.
- RGBA radius-one blur source migration (2026-11-03): the archived adapter used dedicated direct RGBA horizontal/vertical vector passes for BoxBlur, BoxBlurXY, and GaussianBlur whenever both axes resolved to a radius-one window, including repeated passes. Restored that strategy under the exact RGBA mode owner, retained scalar border arithmetic, vector/scalar-tail receipts, and the small-uniform identity copy, and routed eligible requests through it after injected architecture validation. L radius-one BoxBlur/GaussianBlur formulas and pass receipts were also source-compared with their archived kernels. Generic native-byte blur, remaining LA/RGB radius-one routes, and exact-mode admission/fallback comparison remain open; output and selected/fallback receipts stay TODO in `pillow-rs#3ptb`. Source-only; no build, formatter, checkstyle, lint, test, parity, or benchmark ran.
- LA GaussianBlur radius-one source comparison (2026-11-03): compared the exact-mode `[L, A]` horizontal and row-major vertical passes with the archived kernels. Q24 weights, clamped edge samples, three byte-rounding boundaries, radius admission, small-uniform copy, and per-pass receipts match. Restored direct unaligned full-vector loads for complete vertical blocks while retaining zero-padded partial loads. Generic native-byte blur, RGB radius-one, and exact-mode admission/fallback comparisons remain open; output and selected/fallback receipts remain TODO in `pillow-rs#3ptb`. Source-only; no build, formatter, checkstyle, lint, test, parity, or benchmark ran.
- Generic native-byte blur source comparison (2026-11-03): compared BoxBlur, BoxBlurXY, and GaussianBlur planning and execution with the archived CPU/SIMD owners. Fixed-point parameter conversion, the rolling accumulator and clamped edge indices, per-pass Q24 rounding, horizontal-before-vertical ordering, transpose-based vertical passes, identity/small-uniform copies, exact-mode reconstruction, and serial transpose match by inspection. Restored the archived no-fraction vector block so zero fractional weights avoid constructing and processing fractional lanes. The non-parallel archived transpose uses the same scalar interleaved loop; parallel scheduling is gated separately and remains outside `pool_cpu`. Mode admission, requested/selected architecture fallback, output bytes, and operation receipts remain pending in the frozen CPU parity matrix under `pillow-rs#3ptb`. Source-only; no build, formatter, checkstyle, lint, test, parity, or benchmark ran.
- RGB radius-one blur source comparison (2026-11-03): compared the maintained exact-mode RGB BoxBlur and GaussianBlur architecture strategies with archived `pool_simd` kernels. BoxBlur retains the three-row horizontal ring, fixed-point vertical combine, replicated top/bottom rows, and the one-row horizontal-only path. GaussianBlur keeps the f32-derived radius-one weights and six separately rounded passes; RGB byte offsets preserve channel identity across vector loads and scalar borders. `BoxBlurXY` uses this specialization only for two radius-one axes and one pass; other admitted requests reach the shared native-byte recurrence. The small-uniform native-copy proof and prepared architecture validation remain ahead of execution. RGB output, mode admission, fallback choice, and receipts remain pending CPU parity under `pillow-rs#3ptb`; no build, formatter, checkstyle, lint, test, parity, or benchmark ran.
- I;16 Transform architecture source comparison (2026-11-03): compared the four exact typed mode owners with archived CPU and SIMD paths. The architecture guard admits nonempty checked storage, affine requests with six finite coefficients, and nearest or bilinear filters, matching the archived luma16 affine vector gate; other requests retain each exact mode's Default fallback. Nearest samples typed u16 values with the archived pixel-center affine mapping. Bilinear uses a scratch byte plane containing the first `width` bytes of each encoded source row, applies the shared affine byte kernel, restores the untouched fill byte in that mode's byte order, and returns the original exact I;16 identity. No intermediate DynamicImage mode conversion is used. Output bytes, endian edges, selection, and fallback receipts remain pending frozen CPU parity; no build, formatter, checkstyle, lint, test, parity, or benchmark ran.
- Filter3x3/Filter5x5 no-interior architecture source comparison (2026-11-03): archived SIMD admission allowed a nonempty exact native-byte image whose filter footprint covered no interior pixels, then returned the unchanged raster via the shared vector-copy kernel with a `native-copy` receipt and matching vector/tail counts. The maintained guards previously rejected these shapes and selected Default, preserving pixels while losing the architecture strategy/receipt. The guards now admit positive dimensions with checked storage when scale is finite and nonzero and kernel coefficients are finite; exact-mode byte leaves, the dedicated LA NEON routes, and the I32 owner copy the original native bytes before any interior-only kernel. Zero-sized input remains ineligible. The shared helper records `native-copy` and matching vector/tail counts. Source-only change; parity for output, selected implementation, and receipts remains pending; no build, formatter, checkstyle, lint, test, parity, or benchmark ran.
- Filter3x3/Filter5x5 native-byte convolution source comparison (2026-11-03): the Default leaves and archived CPU rows use the same bottom-to-top kernel-row order, middle-product start, ordered fused taps, row accumulation order, `offset + 0.5`, truncation/clipping, and untouched borders. The maintained vector leaves match archived SIMD lane grouping for 1–4 byte channels, including the RGB 3x3 stream's three-byte horizontal offsets and packed 2/4-channel blocks. LA binomial 3x3 retains its exact integer ring specialization; small-uniform byte identity now returns the same shared-copy buffer that supplies its `native-copy` receipt, avoiding an otherwise discarded second allocation. The L uniform/binomial, LA binomial/single-outlier, and RGB uniform/BLUR-ring 5x5 specializations have also been compared with their archived kernels. Mode-specific guards and dispatch have been source-traced. The Python facade's load-before-filter call, binding validation before `_apply`, built-in/custom Kernel delegates, lazy exact-mode output, and direct indexed-source validation before palette expansion are source-traced; runtime error order, output, selection, and fallback parity remain open under `pillow-rs#3ptb`. No validation ran.

- ImageOps.pad callback snapshot source review (2026-11-03): Pillow computes `contain(image, ...)` before resolving fill and placement. The binding snapshots the Rust `Image` before Python color/centering protocol hooks; loaded pixel mutation uses `Arc::make_mut`, and pipeline operation chains are persistent, so later callback mutations/replacement cannot alter the saved source graph or its attached palette. Narrowed the Pad TODO to CPU parity for callback-driven pixel/palette mutation, wrapper replacement, and `getdata` identity; no runtime claim is made. No build, formatter, checkstyle, lint, test, parity, or benchmark ran. Source: [Pillow 12.2.0 ImageOps.pad](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L305-L345).

- Pillow lazy header-mode integration review (2026-11-03): the pinned `image-slash-star` revision in `pillow-rs/Cargo.toml` exposes `EncodedImage::raster_mode()` from its inspected-header cache without decoding. `Image::open_bytes_with_formats` maps that identity into Pillow `Mode` and stores it on `Image::Bytes`; `materialized_shared()` remains the first pixel-decode boundary. No `DynamicImage` is constructed during open or mode lookup. No validation ran.

- ImageOps.pad centering comparison source continuation (2026-11-03): Pillow indexes only the axis receiving padding, then evaluates `max(0, min(center, 1))` before multiplying by the padding gap and calling `round`. Exact built-in centering values use equivalent clamp/multiply/round arithmetic; custom numeric values invoke Python min/max, multiplication, and round protocols in order. In-range offsets pass through normalized f64 centering. At this source-review point, signed out-of-gap custom round results and paste-coordinate C-int overflow remained TODOs because the core Pad strategy stored normalized centering; no build, formatter, checkstyle, lint, test, parity, or benchmark ran. Source: [Pillow 12.2.0 ImageOps.pad](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L305-L345).

- ImageOps.pad signed placement source continuation (2026-11-03): the Python adapter now preserves the selected axis's min/max, multiplication, round result, two-coordinate paste-box addition, and C-int parsing, then passes the resolved signed position as a typed Pad pipeline operation. CPU Default/SIMD native-sample and hidden-lane composition clip against the canvas using the same signed source/destination windows; positive, negative, and fully clipped positions share the mode-native carrier. The fused L/RGBX Resize-to-canvas path declines positioned cases it cannot represent. Pad mode × architecture output and selection parity remains pending. Source-only; no build, formatter, checkstyle, lint, test, parity, or benchmark ran. Source: [Pillow 12.2.0 ImageOps.pad](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L305-L345).

- ImageOps.pad filter-callback geometry correction (2026-11-03): the core now retains preflight contain dimensions if Python filter coercion resizes or replaces the wrapped image, queues the current mode's exact Resize strategy to those dimensions, and then applies padding. This matches Pillow's saved contain output size while keeping lazy pipeline composition; ordinary calls retain the combined Pad path. Mode/storage mutation and backing-store identity remain pending CPU parity. No build, formatter, checkstyle, lint, test, parity, or benchmark ran.

- GetExtrema empty-mode ordering (2026-11-03): Pillow 12.2 `ImagingGetExtrema` rejects multiband storage, then returns its zero-size status before sample-type dispatch; the binding maps that status to `None`. Empty I;16L/B/N therefore returns `None`, while nonempty instances reach the mode error. `Image::getextrema_formatted` keeps this empty return before exact-mode support validation and now records the source-order reason beside the branch. Source-only; no build, formatter, checkstyle, lint, test, parity, or benchmark ran. Source: [`GetBBox.c::ImagingGetExtrema`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/GetBBox.c#L141-L239), [`_imaging.c::_getextrema`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L2195-L2227).

- PutDataValueAt and bulk PutData physical-lane preservation (2026-11-03): Pillow `_putdata` routes multiband samples through `getink` and stores its full physical four-byte pixel. The synchronous Rust callback path now updates omitted carrier lanes alongside its logical bytes: LA/La/PA component tuples repeat luma into hidden bytes 1 and 2, packed LA/La/PA retains packed bytes 1 and 2, and RGB/YCbCr/LAB/HSV preserve the default/fourth-component/packed high byte. Bulk PutData carries the normalized hidden-lane payload through the lazy pipeline and reapplies it after the selected CPU mode/architecture kernel; untouched suffix pixels retain their existing payload. GPU compact-carrier PutData is explicitly declined until the GPU result path can preserve lanes. Source-only; no build, formatting, checkstyle, lint, test, parity, or benchmark ran. Sources: [`_imaging.c::getink`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L534-L645), [`_imaging.c::_putdata`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L1622-L1630).
- PutAlpha/PutAlphaData physical-lane preservation (2026-11-03): Pillow updates the alpha band of an existing LA/PA-family image while leaving the other raw bytes in place. Default PutAlpha, Default PutAlphaData, and the shared SIMD alpha-mask kernel now retain the source's omitted lanes while replacing alpha. L-to-LA construction invalidates ImageBuffer's zero-lane metadata, so Paste reconstructs Pillow's repeated-luma lanes from the logical luma sample. Source-only; no build, formatting, checkstyle, lint, test, parity, or benchmark ran. Sources: [Image.putalpha](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L1880-L1929), [Bands.c::ImagingPutBand](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Bands.c#L176-L223).
- Draw serial geometry physical-lane preservation (2026-11-03): Pillow `Draw.c`'s opaque 32-bit point/horizontal-span writers store all physical lanes, while alpha-blended RGB writers update only color lanes. `NativeByteCanvas` now carries existing omitted bytes through its serial geometry writes and materializes producer fallbacks only when a touched lane differs; LA/La/PA and RGB/YCbCr/LAB/HSV strategies provide exact lane counts. Bitmap/Text owners and SIMD span stores remain TODO(pillow-rs#3ptb). Source-only; no build, formatting, checkstyle, lint, test, parity, or benchmark ran. Source: [Pillow 12.2.0 Draw.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Draw.c).
- Pad palette-fill correction (2026-11-03): P tuple/name fills now validate against `Image.new`'s temporary RGB palette and write index zero; an attached source palette is retained on the result without lookup or append. P tuple arity is rejected before component coercion, and one-integer tuples retain scalar overflow behavior. The Python binding resolves valid fills before centering hooks. Integer-list P fills remain a `NO_PILLOW_SUPPORT` extension. Source-only; no build, format, checkstyle, lint, test, parity, or benchmark ran.
- Chops binary and fusion physical-lane preservation (2026-11-03): Pillow's `Chops.c` applies byte operations across the physical pixel row, so compact LA/PA and RGB-family sidecars need the same native byte mapping as logical storage. The Chops plan now attaches sidecar results after Default and selected SIMD execution, computes the composed Multiply+Screen mapping for fused runs, and restores the result after owned Add/Sub in-place batch execution. Constant sidecars remain constant; varying inputs are clipped and mapped by row using each source width. When an input lacks retained sidecar state, binary/fused Chops derives its omitted bytes from the canonical per-mode fallback shared with Paste before applying the operation. Chops Invert records the changed RGB-family fallback byte when no source sidecar exists. These updates keep chained Chops→Paste output consistent without mode conversion or pixel-kernel changes. Source-only; no build, formatting, checkstyle, lint, test, parity, or benchmark ran. Source: [Pillow 12.2.0 Chops.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Chops.c).
- Blend physical-lane preservation (2026-11-03): Pillow `Blend.c::ImagingBlend` interpolates all bytes in each physical row and returns full-image copies for alpha zero or one. The prepared Default and SIMD Blend paths now map retained sidecars with the same float32 formula and copy the selected input's lanes at endpoint alphas; direct LAB Contrast Blend does the same. When an endpoint sidecar is absent, its canonical physical lanes are derived from the mode and logical bytes before blending. This covers BlendModule and the direct LAB Contrast blend helper. The prepared Brightness wrapper now maps its zero-based physical lane blend in Default/SIMD. Prepared Color Saturation and direct Contrast LUT now preserve compact hidden lanes. Sharpness mode owners model the filter boundary/interior sidecar output and final blend for the six compact-carrier strategies, deriving absent source lanes from canonical fallbacks and retaining output lanes when they differ from the output fallback. Source-only; no build, formatting, checkstyle, lint, test, parity, or benchmark ran. Source: [Pillow 12.2.0 Blend.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Blend.c).

- Brightness physical-lane preservation (2026-11-03): Pillow `ImageEnhance.Brightness` creates a zero-filled degenerate image, restores alpha when present, then calls Blend. The prepared CPU Brightness wrapper now applies that float32 zero-based Blend formula to retained omitted lanes after Default or SIMD mode execution. Factor zero writes known zero lanes; factor one preserves existing sidecars; absent RGB-family source lanes are resolved from the canonical 255 padding fallback, while LA-family copies remain implicit because they follow output luma. Source-only; no build, formatting, checkstyle, lint, test, parity, or benchmark ran. Sources: [Pillow 12.2.0 ImageEnhance.py](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageEnhance.py#L76-L89), [Blend.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Blend.c).

- Color Saturation omitted-lane preservation (2026-11-03): Pillow `ImageEnhance.Color` keeps an LA source as its own degenerate image, while RGB's L-to-RGB degenerate supplies physical padding value 255 and `Blend.c` interpolates the full pixel width. The prepared mode-aware wrapper now carries LA lanes unchanged and maps RGB's omitted lane with the native float32 `mul_add` formula after Default or selected vector execution; factor zero can establish a constant 255 lane; for other factors, absent source state remains implicit because both the RGB input and grayscale-base padding lanes are 255. Direct Contrast LUT sidecars now use the prepared ContrastBase; Sharpness's six compact-carrier strategies preserve omitted output lanes using Pillow's filter-border/interior rules and final blend. The user reproduced Pillow 12.3.0 subclass `convert()` Paste behavior and supplied its rationale; preserve it as undocumented compatibility; the input builder targets the repository's Pillow 12.2.0 oracle, while the user's Pillow 12.3.0 reproduction remains separate evidence. Do not contact Pillow maintainers or open external issues. Source-only; no build, formatting, checkstyle, lint, test, parity, or benchmark ran. Sources: [ImageEnhance.py](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageEnhance.py), [Blend.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Blend.c).

- Contrast omitted-lane preservation (2026-11-03): the prepared wrapper now carries each direct mode strategy's already-computed `ContrastBase` and factor through Default or selected SIMD execution, so it does not rescan the source. The shared Contrast helper applies the same float32 `mul_add` mapping to compact LA, RGB, YCbCr, and HSV lanes; factor zero emits the known mode-specific base and factor one preserves input lanes. With absent sidecars, canonical defaults remain implicit: LA lanes follow output luma, while RGB, YCbCr, and HSV padding stays 255. LAB remains on the composed Blend path. Sharpness's mode owners now preserve compact LA/La/RGB/YCbCr/LAB/HSV output lanes when source provenance is known, using Pillow's copied borders, zero-filled filtered interior, and float32 final Blend behavior. Source-only; no build, formatting, checkstyle, lint, test, parity, or benchmark ran. Source: [Pillow 12.2.0 ImageEnhance.py](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageEnhance.py).
- Contrast degenerate-base lane preservation (2026-11-03): the eager `contrast_degenerate()` shortcut now attaches the omitted physical lanes from its selected `ContrastBase` after writing the compact logical carrier. This keeps the base output aligned with the constant converted image for LA, RGB, YCbCr, and HSV, including the retained source alpha byte in LA. Same-pixel-size Paste output parity remains pending. Source-only; no build, formatting, checkstyle, lint, test, parity, or benchmark ran. Sources: [Pillow 12.2.0 Convert.c L/LA/RGB/HSV](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L2420-L2479), [L/YCbCr](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L3439-L3455).
- Mutable DynamicImage byte-view audit (2026-11-03): the four active `as_bytes_mut()` callers are owned Contrast base construction, in-place Invert, PutDataValueAt, and UnsharpMask. Each compact-mode path restores or maps its omitted lanes at the operation owner; the other PutDataValueAt modes carry no hidden lanes. Source scan only; downstream output and Paste parity remain pending. No build, formatter, checkstyle, lint, test, parity, or benchmark ran.

- EffectSpread physical-lane preservation (2026-11-03): the Default strategy remaps omitted lanes through its existing Pillow relocation map. Selected SSE2, NEON, and Wasm SIMD128 strategies preserve varying sidecars with the same per-pixel destination decisions as the active byte kernel, using a cloned starting RNG state so process RNG is not advanced a second time. Constant sidecars stay compact; absent source lane provenance remains absent. Source-only and unvalidated; no build, format, lint, test, parity, or benchmark ran.

- Transform physical-lane source continuation (2026-11-03): nearest affine/projective sampling now carries compact LA/La/PA and RGB/YCbCr/LAB/HSV omitted bytes through the same selected Default or vector sampler; filtered LA-family outputs repeat the final luma into their omitted lanes, and filtered three-band outputs retain Pillow fill lane 3. Transform fill coercion supplies the matching physical fill bytes for scalar, tuple, and named-color inputs. The maintained helper remains mode/architecture-neutral composition after exact-mode execution. Source-only; no build, formatter, checkstyle, lint, test, parity, or benchmark ran.

- ImageOps physical-lane review (2026-11-03): Pillow 12.2 `Point.c` writes only mode-defined bytes for two- and three-band pixels, and `ImagingNew` creates a zero-initialized output. The maintained Invert, Posterize, Solarize, Autocontrast, Equalize, and Colorize prepared owners now record zero in omitted physical lanes after the selected Default/SIMD implementation, including Equalize's identity-LUT path. Expand composes retained source lanes in copied pixels and exact mode-specific Image.new fill lanes across Default and selected SIMD routes. Pixel kernels remain unchanged. Raw-byte follow-on Paste parity is pending. Source-only; no build, formatter, checkstyle, lint, test, parity, or benchmark ran. Sources: [Pillow 12.2 Point.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Point.c), [Pillow 12.2 Storage.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Storage.c).

- Convert family orchestration split (2026-11-03): the family `ops.rs` now keeps the typed support contract, exhaustive source-mode classification, and prepared API re-exports. Prepared Convert execution, concrete mode dispatch, palette output handling, child-route selection, and generic pipeline preparation moved to `color/convert/helpers/prepared.rs`; all existing paths remain re-exported. This is a source-only structural change and does not change algorithms or outputs. The generic raster-only cross-mode P/PA prepared result remains explicitly pending because it cannot carry wrapper palette metadata. No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage ran.

- Resize family orchestration split (2026-11-03): the family `ops.rs` now holds the exact-mode support declaration and its architecture capability table, with prepared Resize execution, nested dispatch, and child preparation moved to `resize/helpers/prepared.rs`. The existing `ops::` API is re-exported unchanged; mode implementations and algorithms are untouched. This is source-only and unvalidated; Resize/ResizeBoxed mode-by-architecture parity and receipts remain pending.
- PutPixel exact-mode input source (2026-11-03): the public scalar writer APIs now derive mode from the owning Image instead of accepting a second `Mode` value; internal helpers carry the wrapper-resolved identity through exact-mode expansion and strategy selection. The Python binding remains separate and resolves the Pillow wrapper's mode before host coercion. Updated the existing Rust caller to the mode-bearing-image API. Source-only; public fault order and mode × architecture parity remain pending. No formatter, lint, build, unit test, parity, or benchmark ran.
- Direct GetPixel support declaration (2026-11-03): Pillow-shaped formatted reads, the fixed RGBA8 Rust/JavaScript accessor, and the already-loaded RGB binding shortcut now expose `GetPixel`/`GetPixelRgba` CPU support declarations and exact-mode result shapes. Moved the existing readers from `image/pixel_access/native_pixel.rs` to `analysis/helpers/pixel_access.rs`; public coordinate validation remains in the Image wrapper, and the RGB shortcut preserves `None` for non-RGB/deferred input. This central helper remains a Default-only raster-variant dispatch; mode-local strategy placement is unfinished. Source-only; receipts, fault order, exact outputs, and mode × architecture parity remain pending. No build, format, checkstyle, lint, tests, parity, or benchmark ran.
- GetPixel exact-mode strategy placement (2026-11-03): all 25 formatted GetPixel results now have mode-owned Default implementations under `analysis/<mode>/default/get_pixel_impl.rs`; each mode `ops.rs` declares its exact result shape and validates the selected Default strategy before calling the native-carrier reader. The family classifier reuses those same mode declarations, and the exhaustive raster-tag dispatcher is orchestration only. Shared sample helpers remain typed to the native carrier; LAB offsets and I/F scalar decoding are separate. The Rust/JavaScript RGBA8 convenience route keeps per-mode CoreOnly support declarations and one shared, allocation-free `DynamicImage` projection. A single-pixel read declares no vector architecture strategy. Output/error/receipt contracts remain pending; this is source-only, with no build, formatter, checkstyle, lint, tests, parity, or benchmark.
- GetPixel strategy source audit (2026-11-03): checked `Mode::ALL` against the formatted/RGBA family classifiers, each mode's support declarations, Default module exports and implementation file, and the exhaustive `DynamicImage` dispatch; coverage is 25/25. The loaded RGB fast path checks the exact mode before coordinates and then selects the RGB mode's declared Default implementation. Removed its redundant second mode check. This source audit does not verify runtime values, fault ordering, receipts, or mode-by-architecture parity. No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage ran.
- Analysis prepared-helper path correction (2026-11-03): four helpers now import support declarations through `analysis::ops` rather than the nonexistent `analysis::helpers::ops` sibling. This addresses a module-resolution error surfaced by the push-time WIP Clippy attempt; no follow-up compilation has run, and the full CPU source review remains open.
- Draw module-boundary correction (2026-11-03): restricted mode-owned SIMD kernel exports to the SIMD subtree, removed unnecessary parent re-exports of the span writer/admission/layout helpers, and imported clipping and I;16 text routines from their owning Draw helper modules. This addresses module-visibility and unresolved-item diagnostics from the push-time WIP Clippy attempt without changing Draw algorithms. Not compiled; Draw review remains open.
- Support and compile-diagnostic corrections (2026-11-03): classified unused Blend/Composite native-vector `InputContract` forms as invalid architecture-only declarations, completed exhaustive non-target request arms for pixel span guards, and made ChannelAccess decline Draw-only route guards whose required facts it does not own. Cached the Rotate output row's pixel count before mutable iteration; removed target-specific unused bindings and one unreachable palette-component fallback. These are source changes based on the failed WIP Clippy diagnostics; no recompilation ran and CPU source review is still open.
- Palette-target Convert routing correction (2026-11-03): `prepare_pipeline` rejected cross-mode P/PA before the Image wrapper could call `PreparedOperation::execute_convert_output`; both eager `convert_prepared_palette` and lazy `evaluate_palette_target_convert` enter through that registry preparer. Moved the rejection to raster-only `PreparedConvert::execute`, before pixel work. The wrapper output route now reaches the selected P/PA source strategy and keeps generated palette/alpha metadata. Direct raster-only execution retains its explicit NotImplementedError. Generic prepared results still cannot carry wrapper metadata; exact P/PA output and fault-order parity remain pending. Source-reviewed only; push-time docs-lint passed, while cargo fmt reports workspace-wide formatting drift and Clippy compilation fails on the unfinished CPU tree. No behavior tests, parity, or benchmark ran.
- Compiler-diagnostic source corrections (2026-11-03): widened re-exported Geometry dimension, Draw SIMD, Eval NEON, and Filter mode-helper items to their family/mode owner scopes; added the Paste request variant to the unrelated pixel-span guard rejections; made the PutAlphaData mask borrow share the prepared image lifetime; added documented, target-gated unsafe NEON token construction; and removed compiler-reported unused imports and locals. These edits follow the previous failed Clippy output, are not compiled, and do not change operation algorithms. CPU source closure remains open; validation is still gated.
- Draw omitted-lane source follow-up (2026-11-03): the previously recorded TODO for direct Bitmap/Text and SIMD span lane preservation is already implemented. The LA, La, PA, RGB, YCbCr, LAB, and HSV mode owners carry their hidden-lane sidecars through direct Bitmap and Text writes; `SimdDrawSpanWriter` applies the same lane updates after its native-byte span kernel, and skips hidden-lane writes for alpha-blended three-band RGB spans as the serial writer does. This closes only the omitted-lane source review; I;16 parity, text ink/palette/error ordering, public Bitmap fast-route behavior, and runtime receipts remain open. No build, formatter, checkstyle, lint, tests, parity, or benchmark ran.
- Filter convolution missing-sidecar composition (2026-11-03): `Filter3x3` and `Filter5x5` now derive canonical physical lanes from compact logical input when the source has no retained sidecar, then preserve border lanes and zero the filtered interior according to Pillow `Filter.c`. Box/Gaussian and rank-style filters preserve canonical missing-lane defaults through their own per-lane behavior. Source-only; no build, formatter, checkstyle, lint, tests, parity, or benchmark ran.
- Rotate nearest missing-sidecar composition (2026-11-03): LA-family nearest output remains implicit because omitted lanes repeat each copied or filled output luma. RGB-family nearest output derives the canonical source padding byte when no source sidecar exists and maps out-of-bounds pixels through the selected Default/vector nearest mapper; if the configured fill padding is already 255, no sidecar is needed. Source-only; no build, formatter, checkstyle, lint, tests, parity, or benchmark ran.
- Pad missing-sidecar composition (2026-11-03): same-size Pad now derives canonical input lanes when a compact source has no sidecar, then composes content and exact fill lanes through the output frame. RGB-family fill borders retain their configured physical pad byte; empty sources become constant fill lanes, and borderless copies preserve the source's optional sidecar state. Source-only; no build, formatter, checkstyle, lint, tests, parity, or benchmark ran.
- Transform fill-lane source trace (2026-11-03): `public_transform_fill` stores LA/La/PA fills as `(L, A, lane1, lane2)`, so `hidden_fill` uses tuple entries 2 and 3; three-band modes use entry 3 for Pillow's omitted physical padding byte. Confirmed the existing mapping matches the public fill normalization and retained the implementation. Source review only; no build, formatter, checkstyle, lint, tests, parity, or benchmark ran.
- Transform nearest missing-sidecar correction (2026-11-03): a compact LA/La/PA source without retained physical lanes can reconstruct in-bounds lanes from output luma, but explicit packed fills may carry different omitted bytes. The nearest Transform owner now samples the canonical source lanes and configured fill through the selected Default/vector geometry when those fill bytes differ from luma; canonical fills keep the implicit repeat-luma path. Source-only; no build, formatter, checkstyle, tests, parity, or benchmark ran.
- Convert prepared-output source audit (2026-11-03): all image-facing cross-mode P/PA callers use `execute_convert_output`, which carries generated palette/alpha sidecars beside the exact `DynamicImage`. `Image::push_op` isolates mode-changing palette conversion in its own lazy pipeline node before later palette-safe operations are appended; lazy materialization consumes and caches that sidecar before converting follow-on operations. The generic registry `execute()` is raster-only and intentionally rejects cross-mode P/PA output before executing pixels. This is an explicit wrapper boundary, not an unhandled image-facing route. Source-only; parity and architecture/fallback receipts remain pending; no checks ran.
- Paste base-source conversion correction (2026-11-03): Pillow's base `Image.paste` converts mismatched sources to the destination mode unless the destination is RGB and source is LA/RGBA/RGBa. The Rust wrapper had skipped RGB→RGBA conversion only for unmasked pastes, allowing a retained RGB padding byte to become destination alpha. Removed this mask-dependent shortcut and its now-unneeded `has_mask` plumbing from source preparation; Python subclass override results continue through the resolved-source path unchanged. Source-only; no build, formatter, checkstyle, lint, test, parity, or benchmark ran. Source: [Pillow 12.2 `Image.paste`](https://raw.githubusercontent.com/python-pillow/Pillow/12.2.0/src/PIL/Image.py#L1757-L1764).

- Paste pre-queue contract validation (2026-11-03): the staged final call now checks the resolved source, mask, and region against the exact destination mode's declared Paste input contract before queuing. Same-pixel-size subclass conversion results remain eligible; unsupported result layouts return the declared mismatch fault at the paste call instead of being deferred to materialization. Prepared execution reuses the same declaration. This closes the source-level deferred-fault gap; output, fault ordering, and selected/fallback receipts remain pending CPU parity. No build, formatting, checkstyle, lint, test, parity, or benchmark ran.

- Draw Bitmap admission and orchestration source correction (2026-11-03): the public Bitmap entry point now validates mask identity through `Draw::SUPPORT`'s declared `DrawInputs.bitmap_masks` and `mask_rejection`, retaining early rejection before size lookup while removing a duplicated hardcoded mode match. Supported direct bitmap strategies still resolve before their mode-specific paths; declared Rejected modes retain their existing direct Rust ordering by materializing the mask before the strategy returns the unsupported-mode fault. The RGB and CMYK fast paths remain behind Default strategy selection. This closes declaration drift and splits strategy orchestration from raster/output installation without changing the examined fault order. Python already materializes the mask before calling core; all Bitmap fast routes, receipts, and runtime parity remain pending. No build, formatting, checkstyle, lint, test, parity, or benchmark ran.

- Draw output metadata preservation (2026-11-03): general Bitmap/Text results, RGB and CMYK Bitmap fast paths, the PA replacement path, and RGB restoration in `Draw::image_clone()` now use a Draw-owned wrapper that retains source format, `Image.info`, EXIF, and palette sidecars while replacing pixels. The prior generic `Image::from_dynamic` calls cleared those fields; Python's `Image.format` and `Image.info` read back from the Rust image. Existing explicit P/PA metadata carriers remain in place. This is source-only; metadata parity and mode × architecture results remain pending. No build, formatting, checkstyle, lint, test, parity, or benchmark ran.

- Draw effective-mode fallback removal (2026-11-03): `effective_mode()` now returns `Result<Mode, PilError>`, using the cached explicit/original Mode when available and propagating `Image::mode_identity()` failures otherwise. Removed the silent RGBA fallback that could parse color or shape defaults under a guessed mode. Color, Bitmap, Text, and shape-ink paths now propagate the result; the constructor remains lazy. Source-only; mode/error-order parity remains pending. No build, formatting, checkstyle, lint, test, parity, or benchmark ran.

- Draw image-clone mode error propagation (2026-11-03): `image_clone()` now avoids mode lookup when the original image is not RGB, and propagates mode resolution errors when deciding whether to restore an explicit RGBA drawing context over RGB. This removes a swallowed mode error without changing the lazy constructor. Source-only; RGB context output/error parity remains pending. No build, formatting, checkstyle, lint, test, parity, or benchmark ran.
- Custom-font Draw continuation (2026-11-03): plain-string multiline custom-font drawing now computes Pillow's line positions and alignment through that font's `getbbox`/`getlength` methods. Each line requests `getmask2` with the effective mask mode, mode-resolved ink, stroke options, fractional start, and returned offset. Coverage uses exact-mode Bitmap; RGBA masks keep their color channels and compose through masked Paste. Native FreeType strings stay on the Rust Text path. TODO(pillow-rs#3ptb): consume existing `ImageText.Text` state/positions, arbitrary `getmask2` arguments, custom-font `textbbox`/`textlength`, and callback-fault/output parity. Source-only; no validation ran.
- Custom-font Draw binding correction (2026-11-03): RGBA masks now pass native `_rust_image` objects to the PyO3 Paste bridge, and `getmask2` fallback is selected only when the method is absent so callback `AttributeError` remains visible. TODO(pillow-rs#3ptb): custom `ImageText.Text` state, arbitrary `getmask2` arguments, custom metrics, callback/fault ordering, and RGB/RGBA mask parity. Source review only; no build, formatting, lint, test, parity, or benchmark ran.
- Autocontrast option source migration (2026-11-03): `PipelineOp::Autocontrast` and the exact L/RGB CPU strategies now carry cutoff pairs, ignored histogram bins, and `preserve_tone`. The CPU histogram uses native RGB-to-L samples for tone-preserving statistics and applies the resulting LUT to RGB without changing image mode. Unmasked native L/RGB GPU batches lower the CPU LUT to the GPU Eval kernel, including nonzero/asymmetric cutoffs, ignored bins, and tone preservation; masked GPU requests with nonzero cutoff, ignored bins, or preserve-tone remain routed away from the GPU histogram path. The Python binding currently coerces cutoff and ignore before mask validation and source histogram materialization, so reentrant coercion, mask, and source faults can still be ordered differently from Pillow. TODO(pillow-rs#3ptb): finish wrapper fault-order handling and verify pixel/fault parity by mode and backend; this source work is not yet verified. Pillow 12.2 reference: [ImageOps.autocontrast](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L62-L152).
- ImageOps support entrypoint relocation (2026-11-03): Autocontrast, Equalize, Expand, and Quantize now expose their operation support descriptors from `imageops::ops`; Autocontrast, Equalize, and Expand pipeline front doors also live there. Mode classifiers and prepared native strategies remain in the operation modules. `OperationId::support()` and registry preparation use the family-owned declarations. Source-only; not compiled or parity-validated. `imageops/ops.rs` is at 1,000 physical lines, the configured maximum; preserve that limit and review opportunities to keep the orchestration file compact.
- ImageOps mode-strategy relocation (2026-11-03): moved Posterize/Solarize support declarations and architecture dispatch out of the family table and into each supported mode's `luma8/ops.rs`, `palette8/ops.rs`, and `rgb8/ops.rs`. The family prepared enums route by exact `DynamicImage` variant and retain the shared hidden-lane output helper; each typed mode strategy receives the preselected `StrategySelection` and dispatches only to its own Default/SSE2/NEON/Wasm SIMD kernels. Colorize's L support declaration, prepared type, and architecture dispatch now live in `luma8/ops.rs`. Kernel algorithms are unchanged. `imageops/ops.rs` is 796 lines. Source review only; no compile, formatter, lint, tests, parity, or benchmark ran.
- Autocontrast mode-strategy relocation (2026-11-03): moved exact L/RGB input guards, architecture declarations, and typed prepared kernel dispatch into `luma8/ops.rs` and `rgb8/ops.rs`. The family Autocontrast module retains exhaustive mode admission, shared histogram/LUT construction, and typed mode routing; hidden-lane composition remains shared. Histogram and LUT algorithms are unchanged. Python option coercion/fault ordering remains TODO(pillow-rs#3ptb). Source-only; not compiled or parity-validated.
- Autocontrast Python fault-order correction (2026-11-03): the facade now gathers the source/preserve-tone histogram before applying `ignore` separately to each 256-bin channel and coercing `cutoff` for that channel, then applies the resulting Rust-generated LUT through the current wrapper image's `point` method after Pillow-compatible output-mode checks. This preserves wrapper replacement and mutation performed by effectful parameter callbacks. TODO(pillow-rs#3ptb): the typed Rust lazy ImageOps API still performs mode admission before histogram execution, and arbitrary Python cutoff arithmetic protocols are not retained by the current f64 conversion. Source-only; not compiled or parity-validated.
- ImageOps.expand border correction (2026-11-03): `_expand_border` now broadcasts tuples whose length is not two or four, matching Pillow's border normalization before margin arithmetic. Effectful size/source mutation, eager allocation, and dirty/raw palette behavior remain under the source TODO. Source-only; not validated.
- Equalize mode-strategy relocation (2026-11-03): moved L/RGB/P support contracts, input guards, architecture sets, and prepared kernel dispatch into the corresponding mode `ops.rs` owners. Family Equalize retains exact mode admission, shared mask/histogram orchestration, P palette expansion, and hidden-lane composition. Native mode algorithms and the Pillow-defined P-to-RGB semantic path are unchanged. Source-only; no compile or parity validation ran.

Resize legacy-wrapper source cleanup (2026-11-03): the old whole-image `pil_resize` dispatcher and boxed adapter were already test-only; their integer-mode reducer module was the remaining production-visible piece, and no production callers referenced it. `integer_modes` is now also compiled only for tests. The active prepared Resize strategies still use the shared coefficient, sample, FMA, and pass helpers from `ops::pil_resize`. This is source-only; no build, formatter, lint, test, parity, or benchmark ran.

Pool CPU legacy provenance audit (2026-11-03): migration commit `231674e2e8d6571b912bf055f8e0882eab23b276` is a direct child of restart baseline `3f9dfd4036caa061eabcf2255c9f34a745df9c7e`. Mapped every archived path back to its pre-refactor CPU or SIMD path and compared all 280 Rust files; all existed at the baseline and matched byte-for-byte. A maintained-source scan found no active imports from `pool_cpu::legacy`; `legacy_2` contains no Rust sources. This confirms the archive only; no build, formatter, lint, test, parity, or benchmark ran.

CoreOnly support comment audit (2026-11-03): added nearby `NO_PILLOW_SUPPORT` comments to active CPU contracts for native GetPixel/GetBBox/GetChannel, HSV/LAB/YCbCr Color3DLut, high-depth Pad, Filter extensions, ImageOps internal-mode routes, ExtractBand, and PutAlpha. The comments state why each Rust contract is outside Pillow's surface; support and dispatch are unchanged. A source scan found no unmarked `cpu_core_only` or `SupportScope::CoreOnly` declaration under `pool_cpu/ops`. Source-only; no checks ran. Pillow 12.2 `ImageOps.equalize` converts P to RGB before histogram and LUT application, so the existing native palette strategy needs no `NO_PILLOW_SUPPORT` marker for P. Source: [Pillow 12.2 `ImageOps.py`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L417-L444).

Autocontrast Rust typed-mode fault ordering (2026-11-03): unsupported-mode calls now obtain the public source histogram (or convert to native L before histogramming when preserve_tone is set), validate the typed LUT parameters, and only then return the operation declaration’s mode error. Accepted L/RGB/P requests remain lazy. This closes the typed Rust mode-before-histogram TODO recorded above; Python’s arbitrary cutoff arithmetic and runtime mode/backend parity remain pending. Source-only; no build, formatter, checkstyle, lint, tests, parity, or benchmark ran.

Autocontrast host cutoff arithmetic (2026-11-03): the PyO3 adapter now invokes Python `operator.mul`, `operator.floordiv`, and `int` per channel, preserving arbitrary cutoff arithmetic without converting the object through f64. It evaluates the two tuple components in Pillow order and bounds counts only where the final LUT is unchanged; the Rust helper still owns histogram-edge trimming and LUT generation. This supersedes the pending-cutoff note above. Callback, pixel, and fault-order behavior remains unverified by parity. Source-only; no build, formatter, checkstyle, lint, tests, parity, or benchmark ran. Reference: [Pillow 12.2 ImageOps.autocontrast](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L1817-L1934).

- ImageOps Expand size-mutation source audit (2026-11-03): Pillow computes width/height, resolves fill, and reads/copies palette state before calling `Image.new(image.mode, size, color)`. The `image.mode` argument is captured before `Image.new` compares dimensions; `ImageOps.expand` then calls `out.paste(image, (left, top))` with the current source wrapper. The maintained facade captures the target handle before size checks but later passes only the post-check source handle to the fused Expand operation, whose fill and source modes must match. An effectful size comparison that changes wrapper mode therefore needs distinct target-mode and source inputs, followed by the same Paste conversion or raw-pixelsize route. Canvas allocation and palette mutation/load order also need source-complete handling. This is an implementation gap, not parity-only work; the Python Expand owner carries the corresponding TODO. No code path was changed and no validation ran. Source: [Pillow 12.2 ImageOps.expand](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L2462-L2514).

- ImageOps Expand changed-source implementation (2026-11-03): the prepared fill mode is the target mode captured for Image.new. Expand now validates against that mode even if the source wrapper changes during size comparisons. On a source/target mismatch, the core creates a zero-sized canvas in the captured target mode, applies the mode-owned Expand fill strategy to form the canvas, then queues the current source through the existing Paste strategy. Paste owns the same-mode check, normal conversion, and RGB same-pixelsize LA/RGBA/RGBa exception. P/PA palette state is snapshotted during fill resolution and attached to the target canvas. An explicit source backend pin is reapplied to the composed output graph. The ordinary same-mode path remains the fused Expand operation. This source path is uncompiled and unverified. Eager allocation timing and dirty/malformed raw-palette behavior remain TODO; output metadata and mutation-order parity remain pending.

- ImageOps Expand eager-canvas correction (2026-11-03): size validation now uses the mode captured by `PreparedExpandFill`, matching `Image.new(image.mode, size, fill)` when dimension callbacks change the source wrapper. The new core canvas entry point executes and materializes the target-mode fill before returning; Python then delegates the current source to `output.paste(image, (left, top))`, preserving the facade's source loading, virtual conversion, coordinate parsing, and same-pixelsize behavior; the wrapper then materializes the queued Paste before returning so execution faults remain eager. The source is not loaded to create the canvas. Direct Rust same-mode Expand remains fused. Mode mutation during palette access remains a source gap; dirty/malformed palette behavior, output metadata, and remaining mutation/fault parity are pending. Source-only; no build, formatter, checkstyle, lint, test, parity, or benchmark ran.

- Chops selection-context follow-up (2026-11-03): `ChopsExecutionPlan` now passes its retained `StrategySelection` to all 25 exact-mode entry points, including Default-only owners, fusion, and in-place execution. Architecture-capable mode owners read the selected architecture at their strategy boundary; Default-only owners verify the selected CPU implementation is for their exact mode and Default architecture. The per-mode algorithm and kernel selection logic is unchanged. Static source audit found no remaining bare `Architecture` parameters in Chops mode `ops.rs` files and confirmed all 25 mode owners accept `StrategySelection`. Source-only; no build, formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran. Other operation families still need equivalent end-to-end context review.

- Pad selection-context follow-up (2026-11-03): `PreparedPad` now passes its retained `StrategySelection` through direct Resize-to-canvas composition, vertical append routes, shared mode dispatch, and all 25 exact-mode `pad` entry points. The 13 architecture-capable mode owners validate Pad/backend/mode identity and dispatch using the selected architecture; 12 Default-only owners validate their exact CPU/Default selection. No pixel or fill algorithm changed. Static source mapping confirmed all 25 mode declarations and dispatch arms; no standalone `Architecture` parameter remains at Pad mode entry points. Source-only and unvalidated; archived Pad kernel comparison, native component/error semantics, palette mutation, public direct/eager callers, parity, and CPU runtime receipts remain pending.

- Draw selection-context follow-up (2026-11-03): prepared geometry batches, direct Text/Bitmap composition, and eager native RGB/CMYK Bitmap fast routes now pass the retained `StrategySelection` through exact-mode Draw owners. Sixteen architecture-capable owners validate Draw/CPU/mode identity and select their architecture from that object; four I;16 owners validate the exact Default selection. All 20 supported mode owners also validate Default for direct Text/Bitmap operations. The five unsupported internal high-depth modes remain rejected by their existing support declarations. Pixel algorithms are unchanged. Static source checks confirmed every supported dispatch passes selection and found no bare `Architecture` parameter in Draw mode `ops.rs` files. Source-only and unvalidated; mode/error source review and parity remain pending, and no build, formatter, lint, tests, parity, benchmark, or coverage ran.

- Thumbnail selection-context follow-up (2026-11-03): the prepared Thumbnail plan now passes its retained `StrategySelection` through the I32 mode owner and AVX FMA adapter instead of passing a bare architecture. The mode owner checks Thumbnail operation, I32 mode, CPU backend, and requested backend before dispatch; the AVX FMA token still checks the captured `ExecutionContext` for CPU and hardware support. Default and AVX FMA algorithms are unchanged. Source-only and unvalidated; no build, formatter, lint, tests, parity, benchmark, or coverage ran.

- ImageOps Quantize selection-context follow-up (2026-11-03): the prepared Quantize selection now reaches all 25 exact-mode Quantize owners, including 20 macro-generated native/CoreOnly strategies, L/RGB/P/RGBA mode strategies, P-alpha, RGB-premultiplied, and supplied-palette paths. Each owner validates the selected operation, exact source mode, and CPU implementation before selecting its existing Default kernel. Quantize output algorithms and palette construction are unchanged. Static source mapping found no active bare `Architecture` parameter in ImageOps mode `ops.rs` files. Source-only and unvalidated; no build, formatter, lint, tests, parity, benchmark, or coverage ran.

- Draw Bitmap mask source review (2026-11-03): Pillow 12.2 `ImageDraw.bitmap` loads the bitmap, resolves ink, and calls `draw_bitmap`; `ImagingDrawBitmap` delegates to `ImagingFill2`. The maintained `BitmapMask` and exact-mode writers follow its mask contracts: nonzero `1` coverage, L blending over physical destination bytes with full color coverage only when an alpha destination is zero, RGBA alpha-byte coverage, and RGBa PREBLEND over each physical byte. Compact carriers restore omitted lanes at their mode owners. This closes the Bitmap mask formula source comparison only. PA alpha-aware L behavior, RGBa mask results across every destination, I;16 byte behavior, clipping, fast-route output, metadata, and callback/error ordering remain CPU parity or wrapper-review TODOs. Sources: [ImageDraw.bitmap](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py#L2465-L2482), [ImagingDrawBitmap](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Draw.c#L3760-L3767), and [ImagingFill2 mask paths](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Paste.c#L2193-L2659). No build, formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran.

- TODO(pillow-rs#3ptb): verify Convert's complete exact source/target matrix, compact physical-lane outcomes, wrapper routes, and fault order against the frozen CPU cases. The following physical-lane and LAB entries supersede the earlier per-route implementation TODOs: LAB-to-RGB-family and direct CoreOnly LAB target writers are now implemented. This is a parity TODO; source review and runtime parity remain pending at the Convert family level.

- Convert physical-lane continuation (2026-11-04): exact RGB and YCbCr source owners now copy their retained hidden lane to YCbCr/HSV results; RGBA, RGBa, and RGBX source owners copy physical byte 3 into the compact output sidecar. The shared helper runs after the selected Default/SIMD kernel and leaves pixel algorithms unchanged. Empty and default-valued lanes remain unstored; constant values use a constant sidecar, and varying bytes use per-pixel storage. Remaining LA lane routes outside RGB/HSV, La/PA, other compact targets, and CoreOnly routes remain under `TODO(pillow-rs#3ptb)`. No build, formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran.

- Convert compact-lane continuation (2026-11-04): Pillow's YCbCr-to-RGB and HSV-to-RGB outcomes retain the source's physical fourth byte, and LA-to-RGB/HSV outcomes place source alpha in that byte. LA-to-YCbCr instead synthesizes an opaque fourth byte, which uses the target's canonical value and needs no sidecar. The exact YCbCr/HSV and LA owners now attach only the values that must be retained after selected execution. The helper stores no sidecar when values equal the target's canonical byte, uses a constant sidecar for uniform non-default values, and allocates per-pixel values only when they vary. This follows output behavior needed by later raw-byte operations without changing the logical kernels. Sources: [ConvertYCbCr.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/ConvertYCbCr.c#L1531-L1565), [Convert.c HSV conversion](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L2653-L2856), [Convert.c LA-to-RGB/HSV](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L2497-L2536), and [Convert.c LA-to-YCbCr](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L3439-L3473). Static source review only; no build, formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran.

- Convert palette-lane source review (2026-11-04): Pillow 12.2's palette conversion expands P and PA to RGB with an opaque fourth byte; P/PA-to-HSV also emits opaque X, while YCbCr conversion passes that opaque byte through. The maintained compact RGB-family targets use their canonical opaque value, so no per-pixel sidecar is needed. P-to-PA writes the index into the two omitted lane positions and resolves alpha from palette mode; PA-to-P keeps the index, and P/PA-to-LA writes repeated luma plus the defined palette or pixel alpha. The compact target carriers' canonical lane values match these outputs. Added comments at both P and PA source strategies. Source: [Pillow 12.2 Convert.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c). Static review only; no build, formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran.

- Convert opaque RGB target-lane review (2026-11-04): Pillow's RGBX-to-RGB converter discards X and writes opaque padding; RGBa-to-RGB unpremultiplies color then also writes opaque padding. The maintained compact RGB outputs omit a sidecar, so their canonical 255 lane matches both results. Added source-specific comments to the RGBX and RGBa Default strategies. Source: [Pillow 12.2 Convert.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c). Static review only; no build, formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran.

- Convert opaque fallback review (2026-11-04): direct Pillow routes that synthesize X use 255 for bilevel/Luma-to-RGB/YCbCr/HSV, I-to-RGB/HSV, I;16-to-RGB, CMYK-to-RGB/HSV, RGBA/RGBX/RGBa-to-RGB, and P/PA-to-RGB/YCbCr/HSV. Their maintained compact outputs leave the sidecar empty and use the target's canonical 255 lane. Source-preserving paths remain separate and attach retained bytes at their source owners. Added a comment beside the shared canonical fallback. Source: [Pillow 12.2 Convert.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c). Static review only; no build, formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran.

- Convert two-lane output review (2026-11-04): newly produced LA/La images repeat the resulting luma in omitted byte positions 1 and 2; PA repeats the resulting palette index in those positions. Cross-mode Convert outputs use compact `GrayAlphaImage` values and the canonical fallback duplicates their first sample, while exact same-mode identity clones retain any existing sidecar. P-to-PA resolves visible alpha from palette mode, and PA-to-P keeps only the index. The fallback now states this contract beside the implementation. Source: [Pillow 12.2 Convert.c](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c). Static review only; no build, formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran.

- Resize selection-context continuation (2026-11-03): the shared Resize architecture guard now validates the retained selection against CPU backend, Resize operation, and the injected requested architecture, then supplies its selected architecture to local dispatch. Native-byte and I;16 routes pass `StrategySelection` to the guard; ImageI32 AVX/FMA and NEON kernels and ImageF32 AVX/FMA kernels now receive the selection rather than a bare architecture marker. Algorithms, kernel choice, and outputs are unchanged. Static source audit covered 242 active Resize Rust files and found no raw `Architecture` function parameters or old `require_architecture(context, architecture)` calls; `git diff --check` passed. Source-only; no build, formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran.
- Rotate/Transform sidecar selection-context follow-up (2026-11-03): prepared Rotate and Transform now pass their retained `StrategySelection` into the hidden physical-lane composition helpers. The helpers verify operation, exact mode, and CPU backend before using the selected architecture for nearest-sample lane kernels. Filtering, mode behavior, and pixel kernels are unchanged. Static source inspection found no bare `Architecture` parameters in these sidecar helpers or their prepared call sites; `git diff --check` passed. Source-only; no build, formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran.
- Filter kernel selection-context follow-up (2026-11-03): native-byte extrema, rank, and blur helpers now receive `StrategySelection` instead of a bare architecture. Extrema and rank validate the selected operation, exact mode, and CPU backend; blur also validates its selection against the injected `ExecutionContext` before its radius-one or portable-wide path. All native-byte mode owners pass the same retained selection. Algorithms and outputs are unchanged. Static source scan found no `Architecture` parameters in active Filter helpers; `git diff --check` passed. Source-only; no build, formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran.
- Composite AlphaComposite selection binding (2026-11-03): RGBA architecture binding now accepts the retained `StrategySelection` and validates AlphaComposite/RGBA/CPU identity before producing its local architecture strategy; RGBA mirror fusion binds directly from that selection. LA preparation now validates the same operation/mode/backend identity before matching its selected architecture. Algorithms and outputs are unchanged. The remaining raw `Architecture` parameters in Composite are the const Merge support-declaration builder; Analysis's raw parameter only formats an internal missing-strategy error. Source-only; no build, formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran.

- Convert LAB wrapper-route source audit (2026-11-04): Pillow 12.2 `Image.convert()` has an explicit LAB branch that uses ImageCms. RGB, RGBA, and RGBX sources transform directly to LAB; other modes convert through RGBA first. LAB sources transform to RGB, RGBA, or RGBX. The Pillow CMS bridge maps LAB as three color channels plus one auxiliary byte and copies auxiliary channels when source and target layouts permit. Current Rust Convert-to-LAB instead normalizes every source through RGB and `ConvertLab` accepts only RGB8; the LAB source strategy clones LAB samples into RGB and otherwise treats them as RGB-like values. Existing `NO_PILLOW_SUPPORT` comments and CoreOnly reasons that say Pillow has no LAB route were inaccurate; they are now replaced by explicit TODOs while behavior and support declarations remain unchanged. This supersedes the 2026-11-02 source-target scope correction's claim that nonidentity LAB routes are outside Pillow; that audit checked `Convert.c` and missed `Image.convert()`'s ImageCms branch. Implement the wrapper outcomes with Rust code, then reconcile every source-mode target declaration, preserve alpha/auxiliary outcomes, and verify the full fault order. Sources: [Pillow 12.2 Image.convert](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L1118-L1134), [Pillow CMS mode mapping](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imagingcms.c#L199-L220), and [Pillow CMS auxiliary-channel behavior](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imagingcms.c#L330-L350). Static source review only; no build, formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran.

- Convert-to-LAB source implementation (2026-11-04): exact RGB, RGBA, and RGBX mode owners now run the shared fixed-point LAB kernel on their native three color lanes; the four-byte owners preserve alpha/X in LAB's hidden auxiliary lane. Other accepted Pillow modes use the declared wrapper route: direct RGBA normalization, I;16/F through L then RGBA, and YCbCr/HSV through RGB then RGBA. Operation composition now handles direct lazy routes without inserting an intermediate conversion. GPU admission sends four-byte LAB input to CPU until the GPU shader preserves the auxiliary lane. LAB-to-RGB-family conversion, exact pixel/fault parity, and compile/quality checks remain pending. Source-only; no build, formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran.

- Convert LAB-source RGB-family implementation (2026-11-04): the LAB8 Default strategy now decodes the native signed A/B samples and computes RGB from the D50 Lab values without materializing an intermediate mode. RGB output retains the LAB source's physical auxiliary byte; RGBA and RGBX write it into their fourth byte, matching Pillow's CMS bridge. Support marks RGB, RGBA, and RGBX as Pillow targets; the remaining LAB targets stay explicit CoreOnly extensions with a nearby TODO for native LAB-domain algorithms. Pillow source confirms LAB's three color channels plus auxiliary byte and copies auxiliary lanes for compatible layouts; the LittleCMS source defines the LabV2 encoding and inverse Lab curve used by the Rust kernel. Wrapper selection now keeps LAB-to-RGBX on the direct target strategy. Exact output/fault parity and all CoreOnly LAB targets remain pending. Source-only; no build, formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran. Sources: [Pillow CMS mode mapping](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imagingcms.c#L199-L220), [Pillow auxiliary-channel handling](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imagingcms.c#L330-L350), and [LittleCMS LabV2 decoding and inverse curve](https://github.com/mm2/Little-CMS/blob/master/src/cmspcs.c#L149-L216).
- Convert LAB-source target expansion (2026-11-04): the Default LAB owner now has direct output writers for every declared target. L/LA, premultiplied modes, typed I;16 aliases, CMYK, YCbCr, HSV, I, F, and palette routes consume LAB-to-sRGB samples and write their requested carrier directly; target `1` uses a direct threshold or the existing RGB error diffuser over generated samples. These paths do not materialize an intermediate DynamicImage. They remain explicit Rust extensions in the support contract; matching Pillow's wrapper fallback results and fault order is unverified. The LAB RGB-family auxiliary-lane behavior is unchanged. Source-only; no build, formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran.

- Expand callback-order source review (2026-11-04): a Python `image.palette` override or palette `getcolor` can mutate the source wrapper between `_color(fill, image.mode)` and `Image.new(image.mode, size, color)`. The current prepared-fill path binds one mode before palette callbacks, discards the returned palette object, and validates the fill before `Image.new` size checks. The Python and Rust operation owners now carry a precise TODO to keep initial color mode, later canvas mode, palette callbacks, and raw fill validation at their respective host boundaries. Preserve Pillow-visible output and fault order, including custom palette methods, before closing this path. No behavior change or validation ran.

- Raster generic-copy source review (2026-11-04): `GenericImage::copy_from` copies only the logical `Pixel` values exposed by `GenericImageView`; that trait has neither exact `Mode` nor retained physical-lane provenance. `DynamicImage::put_pixel` preserves the destination's omitted lane state, so `copy_from` follows that behavior and does not copy source-only hidden lanes. Mode-aware physical storage transfer remains with operations such as Paste. This resolves the stale plan TODO without changing raster behavior; lower-level mode-blind `ImageBuffer` mutation effects remain under source review and CPU parity.

- Expand callback-order implementation (2026-11-04): Python now resolves named fills with the initial image mode, invokes `copy()` and `getcolor()` on the P palette object, and captures the target Rust image only after those callbacks. Negative comparisons and ordered C-int/dimension validation precede raw non-string fill conversion. The prepared fill and canvas use the captured target, while `paste()` receives the current source wrapper after size callbacks. The copied palette is attached through `Image.putpalette()` before Paste, including an empty palette for scalar P fills. This closes the prior single-mode/pre-callback Rust palette gap. Dirty/raw palette faults, custom palette metadata behavior, eager-allocation fault order, CoreOnly typed fills, and output parity remain pending; no validation ran.

- Expand palette source correction (2026-11-04): Pillow's ImageOps checks for a palette object regardless of P versus PA, snapshots it as a fresh `ImagePalette` from `image.getpalette(mode)`, resolves only 3/4-component tuple fills through `getcolor(color)` without the source image, and attaches `palette.palette` with the captured mode after `Image.new`. The Python facade now follows those callback and metadata stages for P and PA; the `Image.palette` property reads Rust palette header/sidecar metadata without materializing pixels, while `Image.getpalette()` remains the eager loading boundary. This supersedes the prior callback-order implementation note's copied-object and P-only description. The captured `image.mode` value is now translated to the flat `Mode` enum at the PyO3 boundary and drives the prepared canvas, including subclass overrides. Valid mode-override output parity, invalid-mode error ordering, and P/PA tuple-fill faults when a subclass suppresses `image.palette` remain pending parity. Dirty/raw/malformed palette faults, reentrant palette callbacks, CoreOnly fills, eager-allocation fault order, and output parity also remain pending. Source: [Pillow 12.2.0 ImageOps.expand](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L2485-L2512). No build, formatter, checkstyle, lint, tests, parity, or benchmark ran.

- Expand target-mode implementation (2026-11-04): `imageops.expand` captures the Pillow `image.mode` value before effectful size comparisons. The PyO3 boundary performs ordered C-int extraction, translates the captured spelling to the flat `Mode` enum, validates the exact-mode allocation, and passes that mode through fill preparation and canvas allocation. The prepared target stores the flat `Mode` selector only. Subclass mode overrides now select their requested target mode. Valid mode-override parity, invalid-mode fault ordering, P/PA tuple-fill errors when a subclass suppresses `image.palette`, and runtime output parity remain pending. Source-only; no validation ran.

- Expand no-source-palette fill continuation (2026-11-04): when the host palette callback leaves an integer RGB(A) tuple or list for a P canvas, the Rust strategy now builds the temporary RGB palette and fills palette index zero, matching the public `Image.new` result. PA three- and four-component tuple/list fills now fail with Pillow's distinct tuple-versus-list type messages; other P component tuples retain the single-band fill error. The PyO3 operation bridge also rejects noninteger P tuples and every 3/4-component PA tuple before value coercion, matching Pillow's arity-first fault. One/two-component custom tuple coercion, custom `getcolor` return behavior, dirty/raw palette faults, malformed palette metadata, CoreOnly fills, and runtime parity remain pending. No build, formatter, checkstyle, lint, tests, parity, benchmark, or coverage ran.

- Autocontrast CoreOnly P strategy (2026-11-04): the Rust support declaration now exposes native-index P Autocontrast with Default and selected vector kernels and preserved palette metadata. Pillow's Python facade continues to reject P before the point operation, so its public compatibility behavior is unchanged. This extension has source review only; compilation, runtime results, and fallback receipts remain pending.
