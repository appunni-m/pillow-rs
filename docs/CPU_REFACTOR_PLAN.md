# CPU mode and architecture refactor plan

Status: active. The baseline inventory is captured in
[CPU refactor inventory](CPU_REFACTOR_INVENTORY.md). The original CPU and SIMD
trees are preserved under pool_cpu/legacy; per-family mode and kernel contract
review continues from that archive before each maintained implementation is
written.
Baseline: `3f9dfd4036caa061eabcf2255c9f34a745df9c7e`.
Issue: `pillow-rs#3ptb`; related parity findings: `pillow-rs#brg0`.

## Outcome and scope

Transform CPU execution completely into the mode and architecture structure
defined by [Implementation structure](IMPLEMENTATION_STRUCTURE.md). That guide
is the sole placement authority; this plan defines work order and acceptance.

The maintained implementation is Rust-only. Do not add C or C++ source or
implement image operations in those languages. Comments may describe Pillow's
C implementation when that behavior is the compatibility reference; such
references do not authorize a C implementation or dependency.

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

Implementation starts from this baseline with Stage 0. The user has authorized
the ordered stages below. Do not repeat the reset or discard work created after
the baseline. Keep `pillow-rs-bad` as reference.

## Current pending matrix

Baseline: `3f9dfd4036caa061eabcf2255c9f34a745df9c7e`; continue from the current
`develop` HEAD and preserve all later WIP commits. The CPU source migration
remains in review and is not ready for validation gates. See the append-only
checkpoint notes below for recent source changes and explicit remaining TODOs.
WIP commits `5a5e4ecce` and `857080498` stage Point callbacks outside image
borrows and align GetPixel load/tuple-conversion order with Pillow. Existing
callback and input-order paths have received source review only; the full CPU
implementation and its validation gates remain incomplete.
The direct-instruction reachability reviews for Paste, Color Convert, and
Composite found those instructions behind architecture and target-feature
gates. The reviewed operation support declarations constrain selection by
mode, architecture, and operation guard; executable strategies consume the
prepared selection and retain same-mode Default fallbacks. The broader audit of
instruction-free architecture wrappers and other CPU families remains open.
The maintained-source scan found 27 direct architecture-instruction files in
nine operation families; their support/dispatch paths have now been traced.
This does not close the broader module-declaration or prepared-selection audit.
Recent WIP source audits include complete Transpose mode/architecture
declarations, ImageOps Invert mode scope, and the ImageOps point/histogram
strategies. Invert now declares Pillow's P and high-depth rejection contracts
in operation support, records Rust-only modes as CoreOnly, and leaves
output/fault parity pending. Posterize/Solarize/Autocontrast's P paths are
CoreOnly, while Equalize's P-to-RGB transition is Pillow-defined. The
Expand also has an exhaustive 25-mode strategy map with mode-owned fill rules.
The instruction-free wrapper and all-family prepared-selection audits remain open.
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
canonical opaque X value; no CMYK source lane is copied. Pillow's LAB wrapper
route is now represented for conversion into LAB: RGB/RGBA/RGBX use direct
mode-owned Default strategies, while other Pillow modes use explicit
RGBA normalization and the base-mode routes required for I;16, F, YCbCr, and
HSV. The direct wrapper composition now remains lazy, and GPU admission keeps
four-byte LAB inputs on CPU until its shader can preserve their auxiliary
lane. LAB-to-RGB/RGBA/RGBX now has a native serial transform and preserves the
auxiliary lane. The declared Rust-only LAB target routes now write L, LA,
premultiplied, bilevel, palette, I;16, CMYK, YCbCr, HSV, I, and F outputs
directly from generated samples without materializing an intermediate image.
The Convert-to-LAB composition now uses an exhaustive match over `Mode` to
declare each source normalization route. PA-to-La and PA-to-RGBa use the
RGB-base opaque-alpha result in direct palette-sample loops; PA-to-RGBX uses
Pillow's direct palette route and retains PA alpha. PA declares RGBX and has
no CoreOnly targets. Exact outputs, mode-transition, metadata, fault-order,
and CoreOnly parity remain pending. Official source references and the
remaining Convert
mode/target matrix are in the [CPU refactor inventory](CPU_REFACTOR_INVENTORY.md).

Convert routing clarification (2026-11-04): the exact source-mode support row
and requested target select the Rust strategy directly. Pillow base-mode retry
behavior is used only to derive expected outputs and Pillow support; ordinary
Convert strategies consume the original source raster and write the final target
without materializing an intermediate L, RGB, or RGBA image. LAB remains an
explicit ImageCms composition selected by the source/target route. A Default
architecture fallback means only that a requested architecture has no declared
kernel. Output, fault, palette, and selection parity remain pending under
`pillow-rs#3ptb`.

Reduce's top-level dispatch has now been source-reviewed across all 25 modes.
Seven byte modes and Pillow `F` declare vector routes; the remaining modes use
their typed Default owners. Prepared execution retains the selected mode and
applies mode-specific physical-lane behavior after reduction. Arithmetic,
fault ordering, palette metadata, architecture reachability, and parity remain
open; see the Reduce checkpoint below.

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

Paste raw-storage audit (2026-11-03): Pillow's Python wrapper normally converts a mismatched source, except it skips conversion when the destination is `RGB` and the source is `LA`, `RGBA`, or `RGBa`; its source code also trusts a subclass's virtual `convert()` result without checking the returned mode. The RGB fast path has a documented performance rationale: Pillow's changelog says premultiplied `RGBa` paste avoids multiplication/shift work and is typically twice as fast as ordinary RGBA paste, and records removing an extra conversion for `RGBA`/`RGBa` pasted onto `RGB`. The C `ImagingPaste` primitive checks dimensions and physical `pixelsize`, then copies or blends every physical lane without checking mode identities. Pillow 12.3.0 source and the user's local reproduction establish this as observable implementation behavior, not a documented guarantee: a subclass override may return a different mode and the native paste path proceeds when physical pixel sizes match. Base-image conversion remains before backend strategy selection except for that RGB shortcut; a resolved override result is passed through as returned. The 1-byte and 4-byte source strategies now map physical layouts and retain omitted lanes on compact Rust carriers for Paste/Composite; PutPixel, synchronous and bulk PutData, and LA-family PutAlpha/PutAlphaData preserve their affected lanes; mode-owned Draw Bitmap/Text writers and Default/SIMD geometry spans preserve their affected lanes; crop and transpose-family outputs now remap retained lanes with their pixels. Binary and fused Chops compute omitted bytes from canonical per-mode fallbacks when an input lacks a sidecar, across Default, SIMD, and in-place batch routes. Blend applies Pillow's raw-byte float32 interpolation to retained lanes and resolves a missing endpoint through the canonical per-mode fallback. Brightness applies its zero-image Blend formula to retained lanes in the prepared Default/SIMD wrapper, resolving absent RGB-family input state from the canonical padding fallback. Prepared Color Saturation and direct Contrast LUT now preserve compact hidden lanes. Sharpness maps omitted physical lanes through Pillow's copied-border/zero-interior filter behavior and final float32 Blend for LA, La, RGB, YCbCr, LAB, and HSV. When input sidecars are absent, it derives canonical source lanes, including per-pixel LA-family luma, and stores output sidecars only when they differ from canonical output fallbacks. EffectSpread now carries omitted lanes through the exact Default relocation map and through the selected vector route's identical random destination decisions without advancing the process RNG a second time. Resize and ResizeBoxed now preserve omitted physical lanes for identity and exact-crop copies and normalize resampled lanes by mode: direct 2-/3-band paths clear unused bytes, while filtered LA reconstructs lanes 1 and 2 from final luma. Reduce preserves lanes on factor-1 copies and clears omitted bytes after actual reduction; prepared Thumbnail and sparse RGB PutPixel/Thumbnail fusion use that rule. ImageOps point strategies record zeroed omitted lanes and Expand composes retained source and exact fill lanes at its prepared mode/architecture boundary; raw-byte follow-on parity remains pending. Transform now composes omitted physical lanes at its prepared mode/architecture boundary, with runtime parity pending; prepared Pad composes Resize or canonical source lanes through each output path, including fill-border bytes; Rotate remaps lanes for identity/discrete requests and composes affine nearest/filter lanes at the prepared architecture boundary; absent source sidecars use canonical input lanes so RGB fill padding is retained, while LA-family lanes follow output luma. Runtime parity remains pending. Direct fused Resize-to-Pad owners are L/RGBX and have no omitted sidecar; compact modes use prepared Resize followed by Pad lane composition. Filter3x3/Filter5x5 derive canonical input lanes when sidecars are absent, then apply Pillow's observed border-copy/zero-interior behavior; BoxBlur filters all physical bytes, UnsharpMask applies the same blend to filtered lanes, and native rank/ModeFilter extensions process omitted bytes independently. Pillow's source shows these writes but does not document why Filter.c zeros omitted lanes. Filter runtime parity remains pending. Same-size Pad borders now compose source lanes from retained sidecars or canonical mode fallbacks, then retain the exact configured fill lanes. The typed I;16 path now accepts same-pixel-size source aliases and translates stored byte order at the Paste boundary; exhaustive byte-order, mask, and clipping parity remains TODO(pillow-rs#3ptb). In the RGBa-to-RGB shortcut, source samples stay premultiplied and are copied/blended as stored; `Convert.c::rgba2rgb_` only applies when `convert("RGB")` is actually called. Changes are source-only and unvalidated. Sources: [`Image.paste`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/Image.py#L1756-L1764), [`Storage.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Storage.c#L1848-L1910), [`Paste.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Paste.c#L270-L329), [`Convert.c`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Convert.c#L2990-L3020), and [Pillow 1.0b2 performance change](https://github.com/python-pillow/Pillow/blob/12.2.0/CHANGES.rst#L2845-L2849).

Pillow paste override compatibility rule (2026-11-03): the user reproduced Pillow 12.3.0 with a subclass `convert()` that returns the original image. Same-pixel-size results RGBA→CMYK, RGB→RGBA, and L→P succeed with raw internal bytes; RGBA→L raises `ValueError: images do not match`. Treat this as undocumented, version-pinned compatibility behavior, not a promise that differently named modes are generally interchangeable. The Python wrapper calls conversion as Pillow does, retains the returned mode without a second mode check, and lets Paste reject mismatched physical pixel sizes. Unmasked Paste copies physical bytes; masked Paste remains a separate per-lane blend contract. User instruction: do not contact or open issues in external repositories. The parity input protocol and builder now express an instance `convert()` override returning the same image, with cases for RGBA→CMYK, RGB→RGBA, L→P, RGBA→L, and masked LA→RGBA lane blending. The shared parity oracle remains pinned to Pillow 12.2.0; the user independently reproduced the behavior on Pillow 12.3.0. No runtime verification has run. This does not change the flat `Mode`/`DynamicImage` architecture.

I;16 Paste alias source audit (2026-11-03): traced all four exact source and destination aliases through their mode-owned declarations and the shared typed-u16 plan/kernel. Unmasked writes translate source stored byte order into the preserved destination mode; the architecture route now performs cross-endian swaps in eight-lane u16 blocks and records those vector blocks. Masked writes select the byte at the clipped source/destination offset; binary masks copy full samples, ordinary masks blend one storage byte, and premultiplied RGBa masks retain the PREBLEND zero-mask source contribution. Mask layouts and extents are checked before pixel execution. The source comparison found no discrepancy; all byte-order pairs, mask modes, clipping cases, same-pixel-size override behavior, and route receipts remain parity TODOs. No validation has run.

Raw-lane producer audit (2026-11-03): Pillow physical lanes are producer state, not derivable from `Mode` alone. The LA unpacker writes `[L, L, L, A]`; RGB unpack paths write a fourth padding lane of 255. `Image.new` with no explicit color starts with zero fill. A three-component RGB tuple gets a 255 fourth lane; four-component tuples and packed integers retain the value produced by `getink`. Two-component LA tuples repeat L into the omitted lanes, while packed integers retain their raw component lanes. Raw same-pixel-size Paste can then replace every physical lane. The Pillow-owned `ImageBuffer` now retains those omitted bytes in an optional compact payload; encoded decode and `frombytes` attach decoder lanes, `Image.new` distinguishes omitted color from an explicit fill, and raster crop/flip/rotate/transpose remap them. Paste and Composite carry those lanes through raw copy and masked blend. Masked LA Histogram reads retained physical lane 1 and uses the historical reconstruction only when no lane payload exists. Mode now owns the Pillow physical pixel size, byte-carrier pixel width, and omitted-lane offsets, while ImageBuffer retains producer-provided lane values. Mode-owned PutPixel Default, ordered batch, and all declared SIMD single-write paths update the hidden payload for compact LA/La/PA/RGB/YCbCr/LAB/HSV carriers. Callback-synchronous `PutDataValueAt` and bulk `PutData` preserve exact omitted lanes for LA/La/PA packed and component values and RGB/YCbCr/LAB/HSV packed, three-component, and four-component values; bulk restoration runs after the selected CPU mode/architecture kernel. Constant and image-backed PutAlpha/PutAlphaData preserve untouched hidden lanes for existing LA-family images across Default and SIMD paths; L-to-LA output uses Pillow's repeated-luma fallback. Mode-owned Draw Bitmap/Text writers and Default/SIMD geometry span writers retain omitted lanes for compact LA/La/PA/RGB/YCbCr/LAB/HSV carriers; crop and transpose-family outputs remap retained lanes with their pixels. Chops Invert and binary/fused Chops transform retained bytes when sidecars are present. Invert records the changed RGB-family fallback byte; binary/fused Chops resolve absent input sidecars through the canonical per-mode fallback shared with Paste. EffectSpread now reuses the selected relocation decisions for omitted lanes in Default and SIMD without consuming another process RNG sequence. Resize/ResizeBoxed now compose omitted lanes at the prepared boundary using identity/crop remaps or mode-specific resample output. Reduce preserves lanes on factor-1 copies and clears omitted bytes after actual reduction; prepared Thumbnail and sparse RGB PutPixel/Thumbnail fusion use that rule. Affine Rotate now remaps identity/discrete outputs and composes affine nearest/filter fill lanes at its prepared architecture boundary; ImageOps point results initialize omitted lanes to zero and Expand composes source/fill lanes at its prepared mode/architecture boundary. Transform also composes omitted physical lanes at its prepared mode/architecture boundary, with runtime parity pending; prepared Pad composes Resize or canonical source lanes through each output path, including fill-border bytes; direct fused Resize-to-Pad owners are L/RGBX and have no omitted sidecar, while compact modes use prepared Resize followed by Pad lane composition. Filter follows the source-reviewed physical-lane behavior documented above. Raw-byte follow-on parity remains pending for these source-reviewed paths. Compact-carrier GPU PutData remains declined until its result strategy preserves these lanes. The generic ImageBuffer mutable APIs continue to invalidate hidden lanes because they have no Mode context. The flat `DynamicImage` tag remains the only Mode identity; this is payload storage, not a second selector. See Section 2.2 of [Implementation structure](IMPLEMENTATION_STRUCTURE.md). Sources: [`unpackLA`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Unpack.c#L2924-L2937), [`unpackRGBL`](https://github.com/python-pillow/Pillow/blob/12.2.0/src/libImaging/Unpack.c#L3306-L3317), [`_imaging.c::getink` and fill](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L534-L645). This is an unverified source implementation; no validation has run.

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

### Ordered completion gates

| Order | Gate | Status | Remaining work | Unblocks |
| ---: | --- | --- | --- | --- |
| 1 | CPU family implementation and source review | In progress | Finish every family row below: use archived paths as diagnostic references, then close exact-mode contracts, architecture routes, callers, fallback behavior, and output identity against public outputs and fault contracts. Internal algorithms and representations may be improved or replaced. Keep a TODO at each source owner for any path still pending. | Gate 2 |
| 2 | Cross-cutting source closure | In progress | Complete Mode ownership and palette transitions; trace injected Backend/Architecture through direct, lazy, eager, fused, nested, fallback, and terminal paths; make operation support views match admission and execution declarations. Supported lazy outputs use each operation's `OutputModeRule`; Convert, ConvertMatrix, Merge, RemapPalette, Color3DLut, ConvertLab, Grayscale, Colorize, and EffectNoise declare their rejected-route lazy output in `OperationSupport` instead of a planner-side mode switch. The source audit found no other rejected source-mode route that exposes a changed raster result before its error: mode-preserving routes retain the source identity; rejected Equalize has no raster result; PutAlpha wrappers reject those source modes before queueing; raw invalid `push_op` descriptors retain their deferred operation error without claiming a mode. Verify these wrapper metadata and error outcomes in the frozen CPU parity gate. PutData's payload contract is consulted when its payload differs from the source identity, and prefix reuse requires the planned output to equal the exact source mode. Source review has covered every active buffer-preserving `DynamicImage::with_mode` caller; the test-only resize compatibility helper now fails on an incompatible result carrier instead of silently preserving a mismatched tag. Both cached and uncached pipeline evaluation now check that a declared output mode equals the producer's exact `DynamicImage` variant; resolve any resulting mismatches at the operation strategy or planner, and finish the source audit for remaining declared and planned outputs. | Gate 3 |
| 3 | Archive, dead-code, and size audit | Pending | Source scans found no active imports of `pool_cpu::legacy`, no Rayon/parallel iterator references in maintained `pool_cpu` outside its archive, and no implementation definitions in maintained CPU `mod.rs` files; the remaining definition matches are archived legacy modules. Still account for every archived CPU/SIMD file and baseline declaration; remove only reviewed dead maintained routes; review all first-party functions and files against the 100-body-line / 1,000-line limits. | Gate 4 |
| 4 | Static quality and compilation | Not started; blocked by Gates 1–3 | Run the source-map and structure audits, quality checks, parity build, and available target/feature builds listed below. | Gate 5 |
| 5 | CPU parity | Not started; blocked by Gate 4 | Run the frozen CPU-only parity manifest and resolve every mismatch without changing tests or reducing coverage. | Gate 6 |
| 6 | CPU performance | Not started; blocked by Gate 5 | Compare the frozen CPU workloads against compatible baseline evidence, operation by operation and mode by mode; allow at most 1–5% deviation under the agreed benchmark rules. | CPU complete; Gate 7 |
| 7 | Parallel CPU | Waiting for CPU completion | Move parallel scheduling into `pool_parallel`, then review and verify it as a separate backend and evidence profile. | Later backend work |

“In progress” means source work exists but the gate has not met its exit
condition. “Not started” and “blocked” mean no fresh evidence is claimed.
These are work-status rows, not a manually maintained operation-support
matrix; each operation's support view must come from its own declarations.

### Operation-family source work

| Family / area | Current state | Next pending work | Exit condition |
| --- | --- | --- | --- |
| Chops | Source review complete; runtime unverified | Include all 19 operations, exact-mode owner routes, architecture selection/fallback, P/PA palette retention, lazy output identities, compatibility routing, and receipts in CPU parity. Resolve every mismatch without changing the frozen cases. | Source-to-target map and caller/error-order review are complete; operation × exact Mode × Architecture parity and receipt cases pass. |
| Composite | Source review complete; runtime unverified | Run CPU parity for CompositeModule, Merge, and BlendModule across accepted/rejected exact modes and source/mask pairs. Verify wrapper error order, exact architecture selection/fallback receipts, blocks, tails, empty paths, and output identity without changing the frozen cases. | Every accepted/rejected exact mode and source/mask pair, wrapper error order, prepared caller, and selected architecture/fallback receipt meets the frozen public output, mode, fault-order, and selection-receipt contracts; archive comparisons remain diagnostic evidence. |
| Enhance | Source review complete; runtime unverified | Brightness byte source review is complete: Default and vector LUT owners share Pillow's float32 blend expression, and binary shift shortcuts are selected after factor conversion. Eleven Pillow byte modes are declared; seven have vector architecture routes and four are Default-only. LA/RGBA preserve uppercase `A`, while La/RGBa and RGBX transform their lowercase/padding samples; I/F, I;16-family, and internal high-depth routes remain CoreOnly. Pending: per-mode CPU parity and selected/fallback receipts, including sample 10 × factor `0.09999999999999999` → 1 on Default and each declared vector route. Contrast exposes 25 routed outcomes, including PA's alpha-band child path; CPU parity and architecture/fallback receipts remain pending. ColorSaturation exposes 25 route entries at its operation boundary: six prepared routes, three successful composed routes, nine Convert+Blend rejections, La and RGBa conversion/guard errors, and five internal-mode conversion errors. Each route records its exact L/LA intermediate; `Image::color_degenerate` reads it from the same declaration used by direct Rust routing. Python Color's eager-base/public-Blend caller is source-traced, and Blend now carries the original source's backend pin when the eager base has none; host/direct parity and receipts remain pending. Pending: freeze enhancement parity, including metadata behavior and exact architecture receipts. Sharpness source review is recorded complete. | Every enhancement route has a verified source map, exact output/rejection contract, and injected context; support reporting agrees with public and prepared routes, and Pillow-defined parity passes without changing cases. |
| Paste | Source review complete; runtime unvalidated | The Rust mode/architecture strategies are source-reviewed. The Python facade handles Image-as-mask shorthand, None/default positioning, and len/endpoint expansion before source work. It loads and mode-converts Image sources or resolves color strings, then loads and realizes the destination, evaluates mask truthiness, and loads/realizes a truthy mask; final box arity/coordinate parsing precedes deferred raw numeric color coercion. Box preparation rejects a two-coordinate/default-position paste early when neither the source nor effective mask is an image. Image-as-mask shorthand is normalized into the ordinary mask path in Python, so its size survives two-coordinate expansion even when final truthiness disables the mask. Built-in tuple/list provenance is retained: lists are rejected without inspecting elements; exact-mode component parsing keeps integer, wide-integer, float, and invalid-type distinctions; correctly shaped byte-mode tuples apply `__index__` in field order after arity validation; `F` color parsing is mode-selected and uses float extraction, while non-`F` scalar classification preserves Pillow's integer-only path. The non-tuple scalar diagnostics now distinguish single-band and multi-band byte modes. The Python facade follows Pillow's box order: Image-as-mask handling, `None` to `(0, 0)`, `len(box)` before source work, two-coordinate source/mask size lookup plus Python endpoint addition, and deferred coordinate indexing at final Paste. After source, destination, and truthy-mask loads, the staged final call follows Pillow's `(iiii)` parser: the nested `(iiii)` parser rejects bytes and non-sequences, checks `PySequence_Size`, then fetches each item with `PySequence_GetItem` and converts it to C-int before fetching the next item; strings and bytearrays follow the sequence path. Deferred numeric colors resolve only after all box coordinates. Source review confirms `load()` realizes dirty P/PA palettes at the same source → destination → active-mask boundaries as Pillow. Source review now shows resolved image sources are snapshotted before destination and mask loading, matching Pillow's captured `source.im`; Python subclasses invoke virtual `convert()` at the same point while base-image conversion remains in the typed Rust Paste strategy. Tracing the two pathways shows that the already-resolved Python subclass result is stored and queued unchanged; `paste_prepared_source` does not call `resolve_image_source`. The ordinary base-image path converts differing modes in Rust before backend admission, except Pillow's `LA`/`RGBA`/`RGBa`-to-`RGB` fast path; the Python facade passes a source subclass's overridden `convert()` result through unchanged. CPU Paste now admits same-width 1-byte and 4-byte layouts on that resolved-source path and reconstructs omitted physical lanes for compact Rust carriers. This follows Pillow's optimization: its C paste primitive checks pixel size, then copies or blends every physical lane. `RGBa`-to-`RGB` remains premultiplied in this route; no unpremultiplication occurs because Pillow skipped `convert("RGB")`. The typed 2-byte I;16 alias layout now admits all four exact aliases and preserves destination Mode while translating byte order; the current source audit now accounts for byte-order translation, mask-byte selection, and clipped coordinates; runtime parity for those inputs remains TODO(pillow-rs#3ptb). `DynamicImage::put_pixel` now preserves retained compact-mode sidecars for its mode-aware logical writes. Generic mode-blind `ImageBuffer` mutable escapes invalidate sidecars, while mode-aware `DynamicImage` writes update them; downstream Paste/Histogram outcomes remain TODO(pillow-rs#3ptb). The final bridge holds a Python handle while deferred parsing runs, acquires the mutable `PyImage` borrow afterward, and retains exclusive access through the core mutation; source/destination aliasing and callback-driven mode/palette mutation still need parity. Noninteger singleton tuples in LA/La/PA now preserve Pillow's CPython `SystemError` from the post-unwrapping `PyArg_ParseTuple` call. The unused eager low-level PyO3 `paste` bypass has been removed. Then run CPU parity for all 25 mode identities and each declared/available architecture. Cover normalized source and mask contracts, clipping, empty regions, P/PA metadata, all seven L-mask NEON strategies, generic vector fallbacks, cross-layout RGB and RGBa routes, instance `convert()` overrides returning the source (RGBA→CMYK, RGB→RGBA, and L→P raw-copy successes; RGBA→L pixel-size mismatch; masked LA→RGBA lane blending) against the active Pillow 12.2.0 oracle; retain the user's independent Pillow 12.3.0 reproduction as separate evidence, I;16 byte-lane behavior, output identities, and selected-implementation/fallback receipts. Include Paste's frozen performance workloads after full CPU parity passes. | Every mode declaration, caller, serial and architecture path, kernel formula, fallback, and output identity has source evidence; parity and performance evidence pass their frozen gates. |
| Grayscale / Convert-to-L | Source review complete; runtime unverified | The Grayscale contract covers all 25 identities, rejects `La`, and routes successful modes through exact-mode prepared strategies with `L` output. The bounded RGB eager ImageOps path and lazy P/PA palette input reach that owner with injected CPU context; Convert reuses exact-mode luma leaves and declared vector routes. The nonstandard-source declarations for CMYK, YCbCr, HSV, P, PA, I, and F all include L and LA, so public Convert dispatch selects their native mode strategies. The unreachable generic luma/luma-alpha fallback and YCbCr projection helper are removed. Source review confirms `ops/convert/nonstandard.rs` no longer stages generic RGB(A) luma conversions; the remaining `pil_grayscale_alpha` call constructs a specific `La` Convert result from its declared RGB child and remains part of the source-target Convert composition audit below. | Include exact output identity, error ordering, architecture selection/fallback, and supported-mode parity in the frozen CPU matrix. |
| ExtractBand / GetChannel / Split | Source review complete; runtime unverified | `Mode::band_count()` owns native band counts for all 25 identities. `Image.getchannel` declares exact selector counts: `1` and `P` preserve their mode identity, P retains its palette, I/F reject before materialization/index validation, I;16* is a marked CoreOnly native identity extension, and internal high-depth multi-band outputs preserve u16/f32 samples. Other multi-band routes use the prepared `ExtractBand` child. The SIMD caller-owned `GetDataBandInto` path forwards the image-resolved Architecture context into its byte executor. `OperationSupport::routes()` now exposes per-mode wrapper choices: identity/palette copies, loaded-L shared-buffer identity (exact-sized bytes, CPU-only image route) with a `native-identity` receipt and no architecture selection before its child fallback, tiny-RGB eager CPU child before its lazy child with explicit Loaded-source, requested CPU/SIMD, and maximum-256-pixel guards, and per-band children for multiband Split. The loaded-L identity declaration also carries the Loaded, CPU-request, and one-byte-per-pixel guards consumed by the wrapper; this no-kernel route resolves no Architecture. `GetChannel` identities in `Loaded` and caches successful at call entry use shared-buffer `native-identity` receipts; sources first materialized during the call keep their existing materialization path. Single-band `Split` reports the shared-buffer path except when cached `Bytes` P materialization creates the palette-index copy, which reports `native-copy`; direct `Paletted` P also reports its native index copy. Neither path selects an Architecture; an explicit GPU request records that these host-native routes did not dispatch to GPU. Lazy identity routes preserve source graph receipts before terminal identity receipts; multiband ExtractBand remains a deferred child selected when used. Split reads the band count from shared materialized pixels without cloning the raster, then branches non-palette mode-changing pipelines from those pixels while retaining the original backend pin; P/PA continue through the palette-aware pipeline boundary. Verify the branch output, metadata, and selection receipts in CPU parity and the affected Split performance workload. Source routing, output identity, metadata, and receipt paths are reviewed. Terminal timing/resource summaries stay scoped to the eager wrapper; source graph paths and selections stay in ordered operation receipts. Freeze CPU parity across all 25 modes and Default plus each non-Default architecture request, including Split branch outputs, metadata, error ordering, fallback receipts, and the affected Split performance workload. | Every public and pipeline route reaches the declared mode strategy or preserved rejection, its exact per-mode wrapper/child path is visible at the operation entry point, and selected/fallback behavior is recorded accurately. |
| Put and Get | Partial migration | Python `getdata`, `putalpha`, `putdata`, `putpixel`, `getpixel`, `getchannel`, `split`, `getextrema`, `getprojection`, and `getpalette` now call `load()` at Pillow's eager boundary; `getpixel` converts `xy` to a tuple only after loading, and its binding accepts signed C-int coordinates, maps negative indices from the right/bottom, then checks bounds. Band/index validation and Python input coercion happen only after loading. This matches Pillow 12.2's `getdata()`, `getpixel()`, `getchannel()`, and `_ensure_mutable()` ordering. PutAlpha/PutAlphaData wrapper mode changes, error promotion, retained Python Image.info synchronization, mask validation, and typed output planning are source-reviewed; PutData load-before-coercion, typed mode/value-kind handoff, length validation, bulk-versus-callback split, and I;16 routed writes are source-traced; PutData P/PA metadata and payload transitions are source-reviewed; PutPixel host coercion now runs without an active PyImage borrow: palette components are parsed before a short palette mutation, coordinate conversion runs without a borrow, and value coercion completes before the final write borrow. Reentrant mode/palette mutation, mode expansion, P/PA metadata, coordinate/error order, and selected/fallback receipts still need frozen CPU parity. ExtractBand/GetChannel/Split caller and receipt-source review is complete; exact mode/architecture, child-selection/fallback, and ordered source/child receipt parity remain pending, as does moving GPU terminal eligibility into its declared support contract. `GetData` contracts and direct band reads consume the same exact `Mode::band_count()` mapping. The loaded single-band byte view now consults the owning Image's resolved backend route before taking its CPU-only zero-copy shortcut. GetData byte-band SIMD eligibility reads the exact-mode ExtractBand architecture declarations through the injected CPU context; CPU Default fallback is source-traced. GPU terminal eligibility remains mode-gated in its wrapper until a GPU support declaration composes this CPU contract. Full-buffer, scalar-only band, and formatted I/F/I;16 reads use image-pinned/active backend policy to create a routed CPU Architecture context and publish selected/fallback receipts. Formatted GetPixel source review is complete across all 25 exact identities: every path matches the DynamicImage variant directly; CoreOnly ImageLumaA16, ImageRgb16, ImageRgba16, ImageRgb32F, and ImageRgba32F return native u16/f32 components, and byte modes avoid the generic RGBA projection. Typed values, bounds, and wrapper error order remain parity TODOs. Lazy source operation receipts are now retained before terminal GetData selection for full-buffer, scalar, formatted, and CPU caller-owned band reads; SIMD/GPU projection keeps its selected ExtractBand child receipt. Eager terminal timing/resource summaries cover only the host read; ordered lazy source operation paths and selections are retained in separate receipts. Callback-preserving PutData now captures the original backend request and CPU Architecture together in one reusable sequence context; synchronous I;16 byte writes update only the supplied row-prefix bytes in place and preserve the rest of each native sample. They use Python's reported sequence length, scale/offset clipping, and the exact mode's byte order, with no full-frame staging buffer. Numeric I/F and I;16 PutPixel writers use the same routed context through exact-mode selection and retain the native Default fallback when no specialized writer exists. PutAlpha support now matches Pillow-wrapper rejections, exposes native-width strategies as CoreOnly, admits those typed modes through the Rust wrapper, and no longer predicts YCbCr/HSV output modes in the lazy planner. `putalpha_data` validates destination-mode support before inspecting the mask. Pillow 12.2 promotes a supported destination before validating an image mask; Rust preserves that promotion on mask-validation errors, while successful calls go directly through PutAlphaData without staging a second raster. Invalid non-image values also leave the promoted Rust mode visible. The PyO3 boundary maps arbitrary-size Python integers to the core clamp range without narrowing overflow. The Python facade refreshes retained `Image.info` after success or error so mode-promotion metadata remains visible. Verify mutation, metadata, load-before-coercion, and error order in CPU parity. RGBa PutAlpha and PutAlphaData unpremultiply directly into the required RGBA output before replacing alpha, with no staging raster; source-alpha edge parity remains pending. Default CMYK PutAlpha and PutAlphaData now write RGBA directly from native CMYK samples through the shared rounded CMYK pixel formula, avoiding an intermediate RGB allocation; declared vector routes remain separate. This scalar owner change is source-only and parity remains pending. Finish the remaining error-order, metadata, and prepared-receipt review against the pinned source. Freeze those receipt contracts and the complete band-count/architecture matrix. | Each direct, callback, and deferred mutation/read path has an exact-mode contract and preserves ordering and output identity. |
| Statistics and Analysis | Source review complete; runtime unverified | `GetExtrema`, Histogram, Entropy, GetColors, Stat, and multiband Split size logical bands from exact `Mode::band_count()`. Pillow-facing `GetExtrema` has an `OperationId`/support descriptor, injected CPU context, and exact-mode Default strategies. Its byte modes scan native samples; I/F and I;16 preserve typed scalar results; five internal multiband modes return native u16/f32 extrema marked `NO_PILLOW_SUPPORT`. The separate public Rust/JS byte-valued `getextrema()` now has a `GetExtremaBytes` support descriptor, injected CPU context, exhaustive mode classification, and a Default owner in every exact-mode folder. It directly scans native carriers and projects typed samples without creating an RGBA raster, while preserving the old byte API's raw I/F byte lanes and RGB32F opaque fourth lane. All 25 mode-owned byte-valued GetExtrema scans now bound sample slices to width × height, ignoring trailing carrier storage. Its per-mode parity remains TODO. Eager terminal timing/resource summaries intentionally cover only the terminal operation; ordered source and mask graph operation paths and selections are preserved separately in operation receipts. Full-call latency remains a benchmark-boundary measurement. Histogram and Entropy each expose a 25-mode support contract and 1/L mask rules; all Histogram scans dispatch to exact-mode Default owners, including direct native u16/f32 binning for CoreOnly modes. `HistogramExtrema` is declared as an operation input for exact I/F modes and the Python wrapper passes it to the native I/F scan; unmasked I/F calls preserve Pillow's float32 scale, truncating bin conversion, and out-of-range sample omission, while masked calls and non-I/F modes ignore the optional range. Entropy prepares its own operation selection and composes the Histogram child under the same CPU context. The existing optional Entropy extrema argument is preserved as a native I/F Rust extension marked `NO_PILLOW_SUPPORT`; Pillow's standard Entropy method remains mask-only. CoreOnly LA16/RGB16/RGBA16 and RGB32F/RGBA32F retain native 256-bin-per-band scans with no converted image or staging samples; RGB32F preserves its legacy four-band result by counting the synthesized opaque alpha directly; these internal multiband routes are marked `NO_PILLOW_SUPPORT`. Histogram, Entropy, Stat, and GetColors preserve source graph operation receipts; Histogram, Entropy, and masked Stat also preserve mask graph receipts in validation order, followed by terminal strategy selection. GetBBox, formatted GetExtrema, and byte-valued GetExtrema preserve source graph receipts before terminal selection. Eager terminal timing/resource summaries intentionally describe only the terminal CPU operation; ordered source/mask graph paths and selections remain in operation receipts. Quantize selects exact-mode L, P, RGB8, RGBA8, and all CoreOnly modes. Each accepted identity has a Default owner in its mode folder. L/P/RGB/RGBA read native buffers. CoreOnly modes pass native typed samples to statically dispatched quantizers without a converted DynamicImage or RGB/RGBA staging raster; RGBa unpremultiplies its RGB channels per sample before alpha-aware octree reduction, RGBX ignores its padding byte; CMYK reads its RGB-projection lanes, while YCbCr/LAB/HSV retain their three native lanes, matching the Rust DynamicImage projection without materializing it; I;16 preserves clipping, and internal u16/f32 modes use the raster FromPrimitive mapping fused into sample reads. Every CoreOnly owner is marked NO_PILLOW_SUPPORT. I quantization reads each little-endian i32 scalar and clamps it to byte grayscale; F reads each little-endian f32 scalar through the raster float-to-byte projection. MaxCoverage and median-cut k-means no longer stage converted pixel lists. Mode × Architecture parity, lazy receipts, and CoreOnly extension evidence remain pending. GetBBox has an operation-level contract and exact-mode Default owners under `pool_cpu/ops/analysis`; native scans replace RGBA conversion for all 25 identities, with CoreOnly u16/f32 scans marked `NO_PILLOW_SUPPORT`. Source comparison against Pillow's `GetBBox.c` confirms logical byte lanes, alpha-only selection, I/F raw-word predicates, and native-u16 I;16 predicates. The previous Rust I;16 byte-prefix scan missed nonzero high bytes and now uses typed u16 samples. Direct GetBBox receipts include selected architecture and fallback reason from the prepared selection; its operation receipt list retains lazy source graph work before terminal selection. The eager terminal timing/resource summary covers only the CPU scan; ordered source graph paths and selections remain in operation receipts. The formatted GetExtrema implementation has been source-reviewed; include it and GetBBox's 25-mode Default and nine non-Default Architecture outcomes in CPU parity, with every non-Default request selecting same-mode Default. Cover I;16 values with a nonzero high byte and alpha-only selection. Split and GetChannel preserve native u16/f32 samples for CoreOnly high-depth bands. Stat now has a 25-mode support contract, exact-mode Default owners, routed context, direct I/F scalar strategies, and native CoreOnly histogram composition without converted rasters. Its masked wrapper prepares the selected Stat operation and Histogram child under one routed context, executes the child through the exact-mode Stat owner, and reduces native u32 bins with the public f64 reducer arithmetic without a widened histogram allocation. Stat wrappers now preserve source and optional mask graph operation receipts before terminal selection. The eager terminal timing/resource summary covers only Stat; ordered source and mask graph paths and selections remain in operation receipts. GetColors has a 25-mode support contract, per-mode Default owners, native-width CoreOnly counting marked `NO_PILLOW_SUPPORT`, and injected context. Its wrapper now preserves lazy source graph operation receipts before terminal selection; terminal summary scope is defined above; expanded parity remains TODO. Mask/error order for Histogram, Entropy, masked Stat, and GetColors has been compared with `3f9dfd403`; image-mask validation remains dimension-before-mode, unsupported scalar I/F Histogram is rejected after mask validation, and I;16 GetColors rejection remains before materialization. Direct/deferred receipts and mode × architecture selection/fallback evidence remain pending. Native analysis readers have been source-audited; no converted staging raster remains in active analysis paths. Fused RGB/CMYK PutPixel→GetProjection terminals retain one prepared `StrategySelection` through execution and operation receipts, including its fallback reason. Default versus architecture execution follows that selection rather than the Backend compatibility label; RGB/CMYK kernels and the CMYK positive-write path consume it without selecting again. Source review complete; mode × architecture parity remains pending. | Analysis output shapes and values follow exact modes; special Pillow scalar behavior, CoreOnly extensions, and all callers are source-accounted. |
| Transpose | Source review complete; runtime unvalidated | Keep all transpose-specific CPU execution serial. The archived byte Flip adapter used native row copying without a separate vector kernel; I;16 retains its real vector Flip. Native-byte transpose accepts the archived logical prefix for nonempty input and exact empty storage; I;16 transpose uses the prefix only for nonempty input and falls back for empty input. Single operations, adjacent-transpose fusion, palette metadata, and GaussianBlur's nested RGBA transpose now have source-traced callers and context flow. Pending: frozen operation × mode × Architecture parity and receipts for empty/trailing storage, every method, same-mode fallback, and canceled runs (`native-identity`, no architecture selection). Keep parallel collection kernels for the later `pool_parallel` stage. | Every method, exact mode, architecture guard, storage boundary, and caller maps to archived behavior; CPU parity and receipts pass. |
| Transform | Source review complete; runtime unverified | Python and JavaScript fill coercion use exact `Mode`. Pillow's public `fill` parameter is unused for built-in transform method IDs; `fillcolor` controls outside pixels. The empty-mesh P path retains source palette metadata; add a frozen parity case using a non-default palette. The Python facade exposes `Transform`, `ImageTransformHandler`, and `SupportsGetData`, dispatches handler objects with Pillow's LA/RGBA premultiplied-image behavior, and resolves `getdata()` providers before calling the typed Rust route. Built-in LA/RGBA filtering stays on the fused native strategy. The prepared boundary now composes omitted physical lanes for LA/La/PA and three-band modes after the selected mode/architecture kernel: nearest sampling follows the declared affine/projective sampler, filtered LA-family output repeats final luma, and filtered three-band output retains its fill lane. Source review accounted for all 20 accepted mode owners, five explicit CoreOnly rejections, declared architecture dispatch arms and files, and same-mode Default fallbacks. CPU parity must cover every mode/method/filter route, custom-handler dispatch, SupportsGetData forwarding, LA/RGBA handler mode conversion, non-default palette retention for empty-mesh P, and requested-versus-actual architecture receipts. | All accepted and rejected modes and caller paths are accounted for. |
| Draw | Strategies migrated; source review in progress | Byte-mode Bitmap mask owners now read the RGBa mask alpha and apply Pillow's `PREBLEND`; L-mask coverage follows the alpha-bearing destination rule, and PA writes its native index/alpha pair directly. The RGB Bitmap fast path delegates to the same exact-mode Default owner, preserving that RGBa behavior. I;16* Bitmap follows Pillow's native byte loops: L blends both sample bytes, while 1/RGBA/RGBa step through the raw row byte prefix; RGBa uses alpha with PREBLEND. I;16* Text blends L coverage against both native sample bytes and keeps RGBA-mask raw row-prefix behavior; neither path projects the full raster through u8. Text dispatch specializes directly over FreeType's coverage or RGBA mask layout without an expanded intermediate mask. CMYK and RGBA Text were source-compared with baseline `3f9dfd403`: their native per-channel blend, fill/mask behavior, clipping, stroke order, and output identities match by inspection; direct coverage reads replace the temporary RGBA mask expansion. Follow-up review corrected integer CMYK Text ink handling: its exact-mode owner now consumes all four packed bytes already resolved by Draw instead of zeroing the upper channels. Draw integer resolution now also preserves Pillow’s low-four-byte multi-band UINT8 packing and low-32-bit I storage after signed-long-long parsing. Pillow source confirms both contracts. It now maps two-band LA/La alpha from packed byte 3, three-band LAB from bytes 0–2, and four-band RGBa/RGBX from bytes 0–3. LA/La text shares Pillow’s zero-alpha rule. RGBA/RGBa L-mask text raises color coverage only for zero-alpha destinations; RGBA-mask text follows Pillow’s `paste` blend, selected from the mask layout rather than stroke-pass state. RGBX blends its fourth byte. Singleton tuples now follow Pillow getink unwrapping before mode resolution, including packed multiband inks and the one-value F path; unsupported single-band tuple arities keep mode-specific errors. Wide-integer conversion, tuple-component C-int overflow, and exact mode output still need frozen CPU parity. The frozen `pillow-rs#brg0` CMYK Text and MultilineText inputs use `fill=None`. Source review confirms the wrapper resolves that to `(255, 255, 255, 255)` for the CMYK context, and the exact-mode owner applies the same four-channel coverage blend as the archived CMYK carrier path. No source-level default-fill discrepancy is identified. Keep these frozen calls as focused parity cases; exact output and boundary behavior remain runtime-unverified. Pillow 12.2 `ImageDraw._getink` resolves P-mode tuples through `ImagePalette.getcolor` before ink validation; lists reach ink validation without palette allocation. The Python Draw facade resolves attached-P tuple and string inks through its Python-owned ImagePalette.getcolor before passing a typed index to Rust; this preserves Python ImagePalette.colors lookup, normalization, and mutation before raw-byte conversion errors. Direct JS/Rust tuple input now resolves all arities with a Draw-owned key cache; incomplete raw palette tails remain pending at the core palette representation TODO. Successful palette-byte changes use a metadata-only Rust replacement. The Python/Rust palette boundary now retains dirty bytes in the cached Python `ImagePalette` and sends Rust only the first 256 complete entries needed while drawing. `_realize_dirty_palette` installs the full raw palette at Pillow load/access boundaries, so oversized palette errors are deferred until realization. Dirty LA raw-palette realization now commits native LA alpha even when `info["transparency"]` is absent; when present, it applies Pillow integer/table transparency into the Rust alpha sidecar and preserves the Python info mapping. It rebases metadata after raw-palette installation and before transparency coercion so setter errors retain Pillow’s mutation order. PA raw-palette construction is explicitly rejected when Pillow has no RGB/RGBA unpacker for the supplied raw mode; this does not restrict PA as a raster mode. Dirty P/PA resize realizes after filter/gap and identity checks. Rotate now prepares and validates its typed request once, realizes the dirty palette, then queues against the updated palette metadata. Thumbnail now preflights geometry, no-op, and same-size output for every public mode before Python filter coercion. Resize-reaching paths normalize once; dirty P/PA palettes are realized only after that validation. Paste now stages Image-source loading/conversion or color-string resolution, destination realization, and mask loading in Pillow order, with raw numeric sources and box coordinates resolved at the final call; TODO remains for custom/reentrant sequence hooks, unusual subclass behavior, singleton tuple diagnostics, and the direct low-level PyO3 Paste entry. Also finish save and transform. Verify Paste malformed-length/error-order and thumbnail dirty-palette cases in CPU parity. JavaScript numeric arrays and direct Rust callers use the core resolver as tuple-like P Draw colors at every arity. Python arbitrary-size integers retain exact sign/magnitude keys; wide integral JavaScript numbers use the same canonical representation, including equality with numerically equal floats. The Draw-owned key cache preserves `ImagePalette.colors` lookup and key insertion before byte type/range errors; allocation failure still precedes that insertion. Valid nonstandard-arity colors update complete core palette entries, while incomplete raw palette tails remain a TODO because the indexed core representation stores complete RGB/RGBA entries. Python Draw mutates its cached `ImagePalette` before passing the resolved index and installs only representable Rust palette entries; `_sync` replaces the raster handle without overwriting the Python-owned byte buffer, preserving malformed and dirty bytes across later draws until realization. The core Draw context starts with native compatibility-metadata reservations, and Python refreshes them from the current mutable `Image.info` before P color resolution; allocation advances past reserved indices but appends palette bytes at the current byte length, matching Pillow 12.2; full-palette allocation chooses an unused index in the established reverse scan order. Lazy inputs keep earlier operations behind a metadata-only layer instead of materializing pixels. The lower-level typed DrawColorInput retains tuple/list provenance and all-arity P tuple resolution; the Python facade handles attached-P tuples through its ImagePalette, while lists remain outside palette allocation. The core cache preserves numeric key equality and key insertion before byte-conversion errors. Allocation availability is checked first; RGB/RGBA alpha normalization happens before lookup, and mixed numeric tuples retain component kinds for ordered type/range errors. Incomplete raw palette tails remain a TODO for the direct core path. Source review of JavaScript numeric-array tuple routing and core P palette lookup/allocation ordering is complete; verify Python tuple/list, alpha, reserved-index, cache mutation, malformed raw-tail, and byte-conversion error behavior in the frozen CPU parity matrix. The shared multiline text layout now follows Pillow 12.2 `ImageText.Text._split`: mode-aware font metrics, horizontal and top-to-bottom line placement, its single-line early path, alignment validation, Unicode-safe anchor indexing, and centered/right/justified offsets. CMYK and stroked RGBA continue through exact-mode raster owners. The centered-anchor, explicit-font-size, three-line-spacing, CMYK, and stroked-RGBA `pillow-rs#brg0` cases remain runtime-unverified until the CPU source-completion gate. PA raw-palette input remains explicitly rejected when Pillow has no RGB/RGBA unpacker for its raw mode. The public Python Bitmap facade loads the mask before resolving fill; direct PyO3 Bitmap calls now invoke the existing Rust image load method before fill parsing, which preserves its GIL-release behavior. The direct PyO3 ImageDraw constructor also loads the destination before parsing an optional mode, matching the public facade and Pillow error order. Text and MultilineText now prepare the default font and validate embedded-color mode before fill resolution, and resolve stroke_fill only when stroke_width is truthy; direct PyO3 calls follow the same order. Finish source review of all load-triggering wrapper routes; verify Bitmap load-before-fill ordering and malformed-arity, over-capacity, Python tuple/list, reserved-index, palette mutation, I;16* byte-order, existing ImageText.Text/custom-font argument behavior, and direct Text error order in parity; Draw support now links the Paste child. The current fast-path predicate is: nonempty mask, an explicit non-CPU backend request, mask `1` for destination contexts `1`/`P`/`L`/`LA`/`RGB`/`RGBA`/`CMYK`/`HSV`, or mask `L` for `L`/`RGB`/`CMYK`/`HSV`; RGBA drawing context over RGB is also eligible for RGB storage. Draw's top-level support now exposes the Paste child in each exact mode's `OperationModeRoute`, with raster/context/mask triples, explicit GPU/SIMD backend selection, and nonempty-mask guards; the wrapper evaluates those same guards before queueing the child. Source inspection confirms the lazy graph records the child as Paste; Draw fast-route parity and receipt evidence remain pending. | Every Draw operation's entry contract, source mode, destination mode, architecture route, metadata behavior, and preserved error order map to native-mode implementations; CPU parity and selection receipts pass. |
| ImageOps | Partial migration | Posterize, Solarize, Equalize, Autocontrast, and Colorize validate through their operation declarations. Autocontrast now carries cutoff pairs, ignored bins, and preserve_tone through its typed request and implements native L/RGB CPU histogram and LUT behavior; preserve-tone reads RGB-to-L statistics while retaining RGB output. The unmasked GPU L/RGB path lowers this LUT to Eval, while masked nonzero cutoffs, ignored bins, and preserve-tone remain TODOs. For other rejected modes, the typed Rust API computes the source (or preserve-tone L) histogram and validates LUT parameters before returning the declared mode error; accepted Rust requests remain lazy; Rust also declares a CoreOnly P-index strategy, while the Python facade retains Pillow's P rejection. The Python binding now evaluates Pillow’s `n * cutoff // 100` and `int()` protocol once per channel before passing bounded integer counts to the Rust LUT builder; the source path and all Autocontrast mode/backend behavior remain unverified by parity. ImageOps Invert declares its wrapper guard separately from the prepared Invert child, preserving immediate wrapper errors and deferred child admission. CropBorder and Expand use exact-mode prepared declarations. Expand retains scalar/component values for I;16 and CoreOnly high-depth modes through a typed CPU pipeline route; its per-mode support contract declares P palette-index/color input and byte-carried fills for PA and other byte modes; the Python adapter resolves palette colors for P and PA when a source palette exists, while high-depth modes use native u16/f32 fills. The Python Expand adapter normalizes borders, resolves named fills against the initial mode, and runs P/PA palette callbacks before capturing the source raster used to derive the typed target mode. It validates dimensions against that captured target before building the filled canvas; if size comparisons replace the source wrapper, Paste receives the current wrapper while the canvas keeps the captured target mode. Palette callback mutation ordering is source-reviewed. The binding converts the captured `image.mode` spelling to the flat `Mode` enum and prepares the canvas from it, including subclass overrides. Dirty/raw palette faults, output metadata, mutation/fault parity, valid mode-override output parity, invalid-mode error ordering, and P/PA tuple-fill faults when a subclass suppresses `image.palette` remain pending. P palette resolution consumes `OperationId::Expand.support()` and its declared fill rule instead of repeating a mode check. Malformed/raw palette behavior, exact output metadata, and mutation/error-order parity remain pending. Source correction (2026-11-03): `PreparedExpandFill.mode` remains the Image.new target snapshot. The binding validates against that mode and materializes the filled target canvas before the Python facade calls `output.paste(image, (left, top))`. This preserves allocation-before-source-load order and delegates current-source conversion, same-pixelsize RGB behavior, and source loading to the existing Paste facade; the result is then materialized before returning, preserving eager paste faults. The direct Rust same-mode Expand API remains fused. Dirty/malformed palette behavior, output metadata, and mutation/fault parity remain unverified. I/F keeps its exact scalar bit-pattern encoded in the existing raw byte carrier. The direct byte-tuple route is unchanged and GPU declines the typed variant. CoreOnly Histogram bins now scan native samples; RGB32F preserves the legacy four-band output without conversion, and its result layout is exposed by the exact-mode support classifier. Draw P tuple/list provenance is preserved, and 3/4-component integer tuples preflight palette-slot availability before byte-range errors. Non-integer tuple values, other arities, allocation mutation order, and parity remain pending. Equalize's P-to-RGB transition is intentional because Pillow converts P to RGB before histogramming and returns RGB; retain it as an operation-defined mode transition, not a reuse cast. | All wrappers and prepared routes share operation declarations and preserve exact mode/palette results. |
| Convert, ConvertLab, RemapPalette | Source review complete; runtime unverified | CPU-owned binary, matrix, I;16-target, palette-wrapper, scalar I/F→RGBX, premultiplied, and RGBX compositions pass the image-resolved Backend/Architecture context into eager child strategy execution; CPU-owned fallbacks retain the original backend receipt. Lazy source materializes through its own pinned/active route before wrapper children run. Convert declares LAB and palette wrapper routes plus exact-mode premultiplied/RGBX outcomes in its OperationSupport; supported premultiplied pairs now write directly from their mode-owned samples without an RGB intermediate, and prepared Convert applies declared wrapper rejections before strategy assessment. Lazy Convert-to-P/PA uses a prepared CPU strategy and caches generated palette, alpha, and wrapper metadata beside its pixel materialization; copy, load, palette access, metadata queries, and palette-preserving follow-on pipelines consume that sidecar without forcing early materialization. Both image-facing cross-mode P/PA callers use `execute_convert_output`; `Image::push_op` keeps palette conversion in its own lazy node, and the generic raster-only prepared path rejects that output before pixel work. CPU parity and architecture/fallback receipts remain pending. Same-mode Convert now makes the target guard ineligible for architecture kernels and selects the mode's Default identity path. RGB→La/RGBa, LA→La, and RGBA→RGBa are Pillow-supported pairs. PA, RGBA→La, high-depth, CMYK, YCbCr, HSV, LAB, and RGBX premultiplied targets are CoreOnly direct mode strategies marked NO_PILLOW_SUPPORT. The Convert source/target/architecture comparison for all 25 source modes is recorded in the inventory. ConvertLab source review confirms the exact RGB8 prepared contract, archived CLUT interpolation and byte packing, explicit Convert normalization, and rejection of other direct input identities; no architecture-specific kernel is declared. RemapPalette source review confirms the public L/P contract, inverse index mapping, fixed P result, palette-alpha wrapper metadata, empty-raster fallback, and declared architecture routes; its redundant unreferenced Default wrappers were removed in favor of the shared native helper. Pillow 12.2 pair scopes are now explicit: I;16→L/RGB/I/F, I;16L and I;16B→L/I/F, and I;16N→L, with identity copies; other declared pairs are CoreOnly. Pending: verify Convert, ConvertLab, and RemapPalette parity and selection/fallback receipts; cover CoreOnly conversion pairs across exact modes and Architecture requests. | Every conversion route has an explicit source/target identity and exact strategy or rejection. |
| Resize, Reduce, Thumbnail, Crop, Geometry | Source work in progress; runtime unvalidated | Resize/ResizeBoxed dispatch all 25 exact modes through prepared CPU strategies. Public and direct prepared Resize validation order and float32 box narrowing match the baseline source. Exact `One` and `ImagePalette8` filter policy now resolves to nearest in the exact-mode Resize owner before direct or nested child selection; contain, cover, fit, scale, and pad reach this same prepared route. Boxed tall-image Resize now composes vertical then horizontal prepared child strategies for Pillow modes; LA/RGBA become their explicit premultiplied Mode for the two passes and return to their original mode once. F32 keeps its specialized tall path. CoreOnly typed modes retain their existing native path. I;16 Reduce scans typed u16 samples and returns each exact source identity without an 8-bit L intermediate. The native u16/f32 reducers widen row coordinates before flattening; Reduce source review remains open: the facade load/error order and P factor-one box path are corrected, the shared C-int parser now preserves arbitrary-size overflow and custom `__index__` errors, while unusual sequence-subclass slot behavior remains a parity TODO; mode × architecture parity is pending. The One byte reducer is a native Rust extension marked NO_PILLOW_SUPPORT because Pillow rejects Reduce for mode 1. Crop source review now covers signed/out-of-bounds normalization, lazy operation routing, injected context through CPU registry preparation, exact-mode Default/architecture selection and fallback, native output identity, CropBorder composition, and empty/error handling; parity remains pending. Thumbnail caller and architecture source review covers public sizing, filter normalization, materialization/error order, exact-mode selection, guarded I32 AVX/FMA streaming with Default fallback, and same-context Reduce/Resize children. The host boundary now keeps an opaque `ThumbnailPlan` across `draft`, filter, and reducing-gap coercion; it floors/unpacks public bounds, performs Pillow's draft multiplication/int conversion, carries returned draft boxes through boxed Resize, and parses custom filter/gap values outside a `PyImage` borrow. Execution uses the preflight dimensions instead of recomputing against potentially changed image dimensions. Source review is complete: geometry stays preflighted, while filter/gap callbacks run before the current image is routed; if a callback resizes the wrapper to the selected target after the resize branch begins, the facade assigns a current-mode identity resize, preserving Pillow's core replacement and prior `getdata` backing-store identity. Verify callback-driven mode/storage changes and `getdata` identity in CPU parity. The Python facade preflights no-op, geometry, and same-size output for every mode from known dimensions before filter coercion; a request that does not reach resize returns without decoding or parsing the filter. Resize-reaching calls normalize the filter, validate reducing_gap, then realize a dirty P/PA palette. The gap now travels in the prepared Thumbnail operation into the CPU mode strategy: Pillow default 2.0 selects the existing reduction path, while None disables it; mode-specific alpha and nearest policies remain selected by the exact-mode strategy. Custom gaps decline direct GPU reduction and retain host control. Mode × architecture, reducing_gap, and dirty-palette parity remain pending. Pad now declares `PadFillRule` per exact mode beside `PadParameters`: byte-carried, P palette index/color, native u16, or native f32. The wrapper reads that declaration before constructing a native typed fill payload. Mixed numeric tuples retain component kinds and follow the existing mode conversion; in Pillow 12.2, P uses its temporary palette path only for all-integer tuples of length three or four, while mixed tuples reach the single-band core fill error path. Invalid P tuple arities are rejected before component coercion, and a one-integer tuple uses scalar extraction so oversized values preserve the overflow fault. Valid integer tuple alpha is checked before RGB byte conversion and the temporary P fill uses index zero. Custom/nonnumeric tuple callback behavior and malformed wide-tuple error ordering remain under source review; dirty/raw palette lookup, allocation, and output metadata remain parity TODOs. Oversized integer tuples and lists are retained for mode-aware resolution. P RGB(A) integer tuple fills now validate byte ranges and preserve Pillow’s non-opaque-alpha error order. P color strings use unclamped RGB parsing and discard alpha as ImageColor.getcolor does. P name and RGB(A) tuple/list fills now retain Image.new's temporary RGB palette when no source palette is attached; a source palette carried by the P image remains authoritative. The PyO3 ImageOps.pad binding now computes contain geometry before Python filter coercion, then validates the filter and resize dimensions in a reusable Rust preflight plan; if filter coercion changes image dimensions, execution resizes the current image to the saved contain dimensions before padding. The Python facade performs the same ordered preflight before realizing dirty P/PA palettes. The core resolves fill before centering. Source review confirms the binding snapshots the contain input before Python color/centering hooks; loaded pixel writes are copy-on-write and lazy pipeline nodes are persistent. Keep callback-driven pixel/palette mutation, wrapper replacement, and getdata identity in CPU parity. The PyO3 adapter now evaluates only Pillow's selected padding axis and preserves Python min/max, multiplication, round, paste-box addition, and C-int conversion before passing signed coordinates through PadPositioned. CPU Default and SIMD Pad kernels clip the native source into the target canvas, and hidden physical lanes follow the same clipped window. Runtime and mode-by-architecture parity remain pending. Wide-integer sign/magnitude extraction now uses base `int` descriptors, so int-subclass overrides of `__lt__`, `__abs__`, `bit_length`, and `to_bytes` do not redirect numeric serialization. Malformed P wide-integer tuples and custom/nonnumeric tuple callback behavior remain under source review; non-P wide-integer coercion is parsed at the host boundary. Dirty/raw palette error ordering and native fill/Resize outputs remain CPU parity TODOs. Pillow 12.2 creates the fill canvas with Image.new, then restores an attached source palette on the padded result; I;16 scalar fill wraps to its low 16 bits in the native sample owner. Python integer tuple/list provenance now reaches mode-aware resolution, including Pillow’s integer RGB(A) list extension for P; other list inputs retain mode-specific rejection, and Rotate forwards list provenance to transform fill. ImageF32 keeps Pillow F values in its existing packed float-byte carrier. CoreOnly RGB32F/RGBA32F accept floating scalar/component fills as a `NO_PILLOW_SUPPORT` extension; their native values pass through the typed pipeline and convert once at the exact F32 CPU owner. ImageOps.expand now carries signed independent margins through exact-mode Default strategies; the symmetric nonnegative case retains the row-copy fast path, SIMD accepts only that shape, and GPU admission declines asymmetric or negative margins; automatic routing may select CPU, while an explicit GPU request remains strict. TODO: freeze scalar, two-value, four-value, negative-crop, and backend-fallback border cases in CPU parity, including arbitrary-precision values, custom __index__/arithmetic, output-size C-int overflow, and color/error-order behavior. The direct byte-tuple Pad API stays intact, and GPU rejects the typed variant so CPU owns execution. Verify I;16 wrap semantics, high-depth component/alpha values, every direct Geometry caller, and architecture fallback in frozen CPU parity. ImageOps.contain, cover, and fit now preflight geometry in core before strict Python filter coercion, then call the wrapper's `Image.resize` with the saved size or crop box and parsed filter code. Reentrant filter callbacks may replace the wrapper's `_rust_image`; the saved geometry is retained while `Image.resize` selects the current image's exact-mode strategy. Full-box identity requests copy the current image after filter validation. Rounded-zero output validation remains at the Resize boundary; runtime mode/architecture parity remains pending. The shared `ops/pil_resize` coefficient and sampling helpers remain in production, while generic full-operation wrappers are test-only; Rayon scheduling belongs to the later `pool_parallel` owner. | Nested and direct operations use prepared child strategies with the same context and preserve exact empty/crop behavior; CPU execution stays serial regardless of the separate Parallel CPU build. |
| Color3DLut and Eval | Source review complete; direct point receipts source-updated | Verify operation × exact Mode × target Mode × Architecture behavior, point error order, and selected/fallback receipts in frozen CPU parity. | All source paths and operation contracts align; then include them in frozen parity. |
| Effects | Source-reviewed; runtime unvalidated | Run CPU parity for all declared mode and architecture routes. Cover generator output modes, Spread/Noise RNG order and identity fast paths, selected implementation receipts, unavailable-architecture fallback, and eager/lazy caller behavior. | The archived source map is complete; every declared mode and architecture route, output identity, RNG transition, and receipt matches on CPU parity. |
| Rotate | Source review complete; runtime unvalidated | The wrapper's validation order and exact-mode force-nearest rules have been compared with the archive. Shared geometry source comparison is complete: coefficient rounding/order, center and translation composition, bounds, expanded-canvas dimensions, and shift match the archived CPU route. The 25 exact-mode owners are mapped to native Default and eligible architecture kernels: `1`/`P` force nearest; `LA`/`RGBA` use alpha-aware filtering; premultiplied, indexed-alpha, padding, and color-space modes keep their stored-channel behavior; `I`/`F` use typed scalar interpolation; the four I;16 identities remain typed-u16 Default-only because the archived byte SIMD coordinate and fill contract differs; internal high-depth identities use native samples and are marked `NO_PILLOW_SUPPORT`. Architecture strategies retain identity and exact right-angle paths for eligible nonempty inputs; empty-source requests select same-mode Default so its fill, geometry, and rejection behavior remain authoritative. Compact LA/La/PA affine nearest paths sample omitted bytes with the selected nearest geometry, filtered output repeats final luma into omitted bytes, and RGB-family filtered output retains the configured physical padding fill; source behavior is reviewed, runtime parity remains pending. The routed context reaches prepared execution across ordinary, palette-safe, and cached-prefix paths; each owner returns its exact `DynamicImage` identity. The Python dirty-palette route validates once before palette realization and checks the prepared plan's exact mode before enqueueing. Ordinary and prepared Rust/Python routes converge on the same lazy pipeline; prepared execution and batch telemetry record the selected strategy and fallback reason before running the leaf. CPU parity remains pending for dirty-palette mutation/error order, empty outputs, per-mode output identity, and runtime architecture receipts. | Every supported mode and same-mode fallback is source-accounted and then parity-verified. |
| Filter | Source review complete; runtime unverified | Run frozen CPU parity for all filter operations, parameters, exact input modes, architecture routes and same-mode fallbacks. Cover Filter3x3/5x5 native kernels, small/no-interior copies, I32 uniform identities and AVX/FMA binomial routing, blur and radius specializations, ModeFilter palette/native-channel behavior through exact-mode support and Default leaves sharing the native-byte kernel, UnsharpMask's nested blur context, rank edge/empty behavior, architecture receipts, and public wrapper error/coercion order. Max/Min/Median/Mode/Rank PyO3 arguments preserve the Python object, use Pillow's numeric defaults, and pass through the shared C-int parser; rank-family size preflight matches Pillow's float32 workspace overflow bound. Explicit-None, integer-boundary, window-limit error-order, and coercion parity remain pending. Their CPU-only eager wrappers retain the requested backend while injecting CPU Architecture, including the SIMD alias when GPU is preferred but has no strategy. Kernel source support includes the 13 admitted byte modes (including `One`), `I`, and exact `I;16*` owners with native-u16 Default kernels; P, F, and internal multiband high-depth stay rejected. F32 Default mirrors Pillow 12.2's in-place comparison/partition selection in safe Rust; NaNs and mixed signed-zero images fall back from vector order to this path, while total-order-compatible samples may use the bounded vector strategy. Pillow's edge-padding-plus-crop rank semantics match nonempty clamped gathers; empty-image rank windows of size 3 or larger return a safe empty result, marked `NO_PILLOW_SUPPORT` because Pillow 12.2 reaches an edge read. No validation has run for this source-complete family. | All filter operations, parameters, input modes, and architecture/build guards are source-reviewed and parity-verified. |

Filter source review now has an explicit wrapper order: the Python facade loads
before filtering; the PyO3 layer checks `_apply` before reading the filter name,
then performs named mode preflight before invoking `_apply`. The order is
source-reviewed only; exact built-in and custom-filter error behavior remains
in the CPU parity matrix.

ModeFilter's active native-byte helper was compared with the implementation at
`3f9dfd403`: it keeps the same `size / 2` neighborhood, skips out-of-bounds
samples, builds one histogram per channel, chooses the lower value on ties via
strict `>`, and retains the center sample unless a value occurs more than twice.
P input is filtered as palette indices and the wrapper retains its palette and
alpha table. The refactored caller now routes through the injected CPU
architecture and records the prepared selection. Mode, palette, and fallback
parity remain pending; this does not close the broader Filter source review.

The shared BoxBlur/BoxBlurXY/GaussianBlur paths remain under source comparison.
Trace each exact-mode route through its support guard, native-byte implementation,
specialized radius-one/radius-seven kernels, identity proofs, and same-mode
fallback against the archived CPU/SIMD adapters. The output and selection
receipts remain pending CPU parity; the active source owner carries a TODO.
The source comparison found that the maintained vector blur's zero-radius and
small-uniform exits copied with `to_vec()` without publishing the archived
`native-copy` path or its vector/tail counts. They now use one checked,
mode-preserving vector-copy helper; RGBA's radius-seven identity proof uses
the same helper after recording its proof scan. This corrects the receipt path
but does not complete the remaining blur source comparison or parity.

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
RGBA GaussianBlur output and selection-receipt parity remain pending; broader
Filter review is still open.

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
three byte-rounding boundaries and direct full-vector loads. Generic
native-byte blur and exact-mode admission/fallback boundaries remain
source-review TODOs.

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
below.

The typed I32 Filter3x3 Default and AVX2/FMA routes have also been
source-compared with the archived implementations. Default preserves the
scalar f32 row order and exact cross-smooth specialization. AVX2/FMA preserves
the four-lane arithmetic order, scalar tail, and copied border; its support
guard requires exact four-byte storage, finite kernel parameters, a complete
four-lane interior block, and at least three rows. The prepared uniform-image
proof uses the matching Default or AVX2 scan before execution. Filter3x3 bytes
and selected/fallback receipts remain pending CPU parity.

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

### Direct analysis and adjacent ImageOps migration matrix

This matrix tracks source migration only. Runtime parity and benchmark columns
remain pending for every row; `Architecture` coverage means `Default` plus all
nine non-Default requests, with same-mode `Default` fallback where no exact
kernel exists.

| Operation / public route | Mode contract and CPU owner | Native sample behavior | Context / caller state | Pending |
| --- | --- | --- | --- | --- |
| Formatted GetExtrema | `OperationId`, exhaustive 25-identity classification, and exact-mode Default owners exist; 22 identities are accepted and three I;16 endian identities retain the Pillow rejection. | Byte, I32, F32, I16, and internal u16/f32 multiband strategies reduce native samples; internal results are marked `NO_PILLOW_SUPPORT`. | Prepared CPU selection reaches the formatted Rust and host formatters. | CPU mode × architecture parity and receipts. |
| Byte-valued `Image::getextrema()` | `GetExtremaBytes` has exhaustive exact-mode support, injected context, and Default owners for all 25 identities; every typed scan is bounded to the declared width × height. | Direct native-carrier scans preserve the byte API result contract without an RGBA raster; typed samples are projected only at the result boundary. | The operation receipt list preserves lazy source graph work and terminal selection; summary timing/resources cover only the CPU scan; source graph paths and selections stay in ordered operation receipts. | Freeze operation × mode × architecture parity for the exact byte-result contract and fallback receipts. |
| GetBBox | Exhaustive 25-identity contract and one Default owner per exact mode exist. | Native byte, I/F raw-word, I;16 u16, and internal high-depth scans. | The operation receipt list preserves lazy source graph work and terminal selection; summary timing/resources cover only the CPU scan; source graph paths and selections stay in ordered operation receipts. | CPU mode × architecture parity, terminal selection receipts, alpha-only and high-byte guards. |
| GetProjection | Exhaustive exact-mode support and prepared CPU strategies exist; RGB may use its declared GPU route. | Each Default/architecture strategy scans the native carrier. | Direct and materialized-pipeline routes now carry one Backend/Architecture policy snapshot; unsupported GPU routes use the selected CPU architecture, including active SIMD fallback. | Verify direct, GPU-fallback, and lazy terminal receipts plus operation × mode × architecture parity. |
| Histogram | `OperationId`, exhaustive 25-mode support, and declared 1/L mask rules exist; all 25 exact-mode Default owners dispatch from the prepared CPU selection. All owners scan only the width × height logical pixels from native typed samples. For masked LA/La/PA, Pillow Histo.c reads physical lane 1 into the second bank; the typed owner reads retained lane 1 and falls back to the canonical repeated first sample when provenance is absent. New/frombytes/decode construction, packed PutData, and same-pixel-size mode-mismatched Paste retain physical lanes in HiddenPixelLanes. This repository has no Arrow image-import API. Audit any producer that drops lane provenance before Histogram; that cross-operation source gap and parity remain pending. The same mode classifier exposes Histogram result bands and bins; RGB32F declares four bands to preserve its synthesized opaque compatibility band. The `HistogramExtrema` input contract names only I/F; Python parses it only for unmasked I/F calls, whose native scan uses Pillow-compatible float32 scaling, truncating bin conversion, and omission of out-of-range bins. I's full-range signed overflow and F's non-finite/out-of-i32-range float-to-int cases are Rust-defined extensions marked `NO_PILLOW_SUPPORT`; their extreme boundaries remain frozen parity TODOs. | Pillow I/F and I;16 retain their special sample rules. CoreOnly modes scan native samples directly into 256 bins per native band: u16 computes `(sample + 128) / 257`; f32 computes `round(clamp(sample, 0, 1) * 255)`, with NaN mapped to the upper endpoint. These calculations produce bin indices directly from each u16/f32 sample; neither samples nor images are converted or staged. | Direct wrapper injects routed CPU/architecture context and records materialized-call selection; Entropy composes this prepared child under the same context. | Complete operation × mode × architecture parity and terminal selection receipts; separately verify CoreOnly extension thresholds (u16 bin edges and f32 clamp, NaN, and rounding boundaries) without treating them as Pillow modes. |
| Entropy | `OperationId`, exhaustive 25-mode support, and 1/L mask rules exist; the prepared Entropy owner composes Histogram. Its pre-existing optional extrema parameter remains a `NO_PILLOW_SUPPORT` Rust extension for unmasked I/F samples. | All mode semantics come from the matching exact-mode Histogram owner, including native u16/f32 bins for CoreOnly modes. | Injected CPU context selects Entropy and its Histogram child. Its operation receipt list preserves source graph, optional mask graph in validation order, and terminal selection; timing/resource summaries cover only terminal Entropy. | Complete operation × mode × architecture parity and terminal pipeline receipts. |
| GetColors | Exhaustive 25-mode `OperationId`/support contract; one Default owner per accepted exact mode, with I;16* rejected before materialization. | `1`/`L`/`P` count native indices; byte multiband modes count raw channels in Pillow slot order; I/F retain scalar hash behavior; CoreOnly u16/f32 modes count native samples without image conversion and return typed tuples. | The operation receipt list preserves lazy source graph work and terminal selection; summary timing/resources cover only the CPU scan; source graph paths and selections stay in ordered operation receipts. | Freeze CPU-only GetColors parity for all 25 modes × declared Architecture requests, including output shape/order, maxcolors, rejection, and fallback receipts. |
| Stat | Exhaustive 25-mode `OperationId`/support contract and one Default owner per exact mode; non-scalar modes compose the Histogram child. | I/F preserve their specialized scalar histogram formulas; byte, I;16, and CoreOnly modes reduce prepared 256-bin Histogram output, with CoreOnly bins scanned from native samples and no converted image. I full-range overflow and F undefined float-to-integer cases use explicit Rust behavior and are marked `NO_PILLOW_SUPPORT`; their boundaries remain unverified. | Direct and masked wrappers inject routed CPU/Architecture context; the masked path executes its Histogram child through the selected exact-mode Stat owner using direct u32-bin reduction that preserves the public f64 arithmetic without a widened histogram allocation. Both preserve source and optional mask graph operation receipts before the terminal selection; terminal timing/resource summaries cover only the Stat operation; source and mask graph paths and selections stay in ordered operation receipts. | Freeze all 25 modes × declared Architecture parity, masked error order, and terminal selection receipts, including scalar I/F extension boundaries. |
| Split | Exhaustive 25-mode `OperationId` contract composes the exact-mode `ExtractBand` child. | P preserves its indexed palette; single-band modes preserve exact identity; multiband children extract native bands, including CoreOnly u16/f32 samples. | The wrapper carries a mode-validated lazy graph; each child selects backend and architecture when materialized. | Verify palette and output metadata, child selection/fallback receipts, and all exact-mode outcomes in CPU parity. |
| ImageOps / Quantize | `OperationId` and exhaustive 25-mode source contract select the fixed P output under injected CPU/Architecture context; all modes have Default, with no architecture kernels declared. | L, P, RGB8, RGBA8, and all 21 CoreOnly modes use exact-mode Default owners under their mode folders. The operation contract lists method-restricted alpha modes and their allowed methods; LA/La/PA, RGBA, RGBa, LA16, RGBA16, and RGBA32F select alpha-aware Default sources, while RGBX uses RGB-only quantization and ignores padding. Pre-materialization validation stays Pillow-RGBA-only to preserve error order. Each CoreOnly owner reads its native storage through a statically dispatched sample source, without constructing a converted DynamicImage or RGB/RGBA staging raster. LA/La/PA and typed alpha modes read native color/alpha samples through an alpha-aware octree source; PA resolves palette colors and combines per-pixel/palette alpha without a staged image. RGBa unpremultiplies each native sample before alpha-aware octree reduction; RGBX uses the native RGB-only quantizer and ignores padding; I;16 keeps its clipping rule; internal u16/f32 modes use the raster FromPrimitive mapping fused into sample reads. Every CoreOnly owner is marked `NO_PILLOW_SUPPORT`. Shared palette algorithms live under `pool_cpu/helpers/quantization`; MaxCoverage and median-cut k-means consume native sources without staging converted pixel lists. | Direct wrapper prepares from the materialized DynamicImage mode, preserves requested Backend, and records exact-mode selection. Lazy source graph receipts are restored before the terminal Quantize selection; terminal summary timing/resources cover Quantize alone; source graph paths and selections stay in ordered operation receipts. Palette-image, method-restricted-mode, and allowed-method constraints are in the operation input contract. | Source review compared the moved palette reducers with the pre-refactor files: median cut, MaxCoverage, octree, and fixed-palette mapping preserve their existing algorithm routes while consuming statically dispatched sample sources; MaxCoverage retains its single-color palette behavior. The exact-mode Default routes and RGB/RGBA palette output projections are source-reviewed. The Python facade still has a TODO for arbitrary palette truthiness and custom `load`/`mode` callbacks (`pillow-rs#3ptb`). Freeze CPU mode × architecture parity, selected/fallback receipts, palette alpha and empty-image cases, including CoreOnly extensions separately. Runtime verification remains pending. |

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

### Cross-cutting source work and evidence gates

| Workstream | Status | Pending work / exit condition |
| --- | --- | --- |
| Mode ownership and boundaries | CPU source review complete; parity pending | `DynamicImage` and `Mode` declare the same 25 exact identities; CPU construction and operation APIs use typed `Mode`, while materialized execution reads the `DynamicImage` variant. Decode/encode translation stays in Pillow-owned codec adapters, lazy output transitions use operation support declarations, and indexed palette state retains the exact P/PA raster identity. Python and JavaScript parse public raster-mode strings at binding boundaries. `ImageColor.getcolor` uses a distinct typed `ColorOutputMode` for Pillow `ImageMode` descriptors such as `I;32` and `I;16S`; these values are not coerced to raster `Mode`. The later GPU compatibility path still carries Pillow spellings in its pending execution metadata and remains a separate backend-stage TODO. CPU source review is complete; frozen CPU parity and receipt verification remain pending. |
| Backend and Architecture context | Incomplete | The shared selector checks the requested Architecture before permitted lower-architecture declarations, preserves `MissingSpecialization` for lower implementations, and carries the selected guard in `StrategySelection` for execution. CPU-only direct Analysis, Quantize, point/eval, eager Filter, Draw, GetData, GetProjection, callback PutData/PutPixel, and the CPU-owned binary/matrix/I;16/palette Convert routes now use the shared image-pinned/active Backend plus CPU Architecture resolver (advisory where the old wrapper swallowed backend-state errors); GPU preference can therefore retain an active SIMD architecture for CPU fallback, while an explicit GPU pin stays CPU Default. Eager receipts treat SIMD as an architecture request: a selected SIMD strategy clears the provisional host-fallback note, while Default/lower-architecture selection records the strategy fallback; GPU-to-CPU stays a backend fallback. Per-operation telemetry now exposes optional typed strategy fallback reasons separately from pipeline-level backend fallback (`None` means selection is unavailable; an empty list means a recorded selection had no fallback), and prepared CPU batch, direct eager, composed, fused, and vector PutPixel batch selection paths record the actual `StrategySelection`. The owned batch Invert path keeps its exact byte-mode eligibility and derives its kernel band count from `Mode::band_count()`. Fused RGB/CMYK GetProjection terminal execution and receipts now consume the prepared selection directly; RGB PutPixel→Thumbnail fusion admission also passes the single Image-resolved Backend/Architecture snapshot into its support guard. Other host terminal paths retain source TODOs for selection and receipt plumbing. Continue tracing injected context through registries, pipeline, eager/direct, fusion, nested, fallback, and terminal edges; audit owner declarations and verify exact, lower-architecture, build/hardware-unavailable, and guard-fallback receipts in parity. |
| Operation support declarations | Incomplete | For every operation, expose all 25 exact modes, inputs/outputs, multi-input constraints, build/architecture requirements, deferred guards, and rejection/fallback reasons at its top-level contract. Use the same declarations for reporting, admission, and prepared selection. |
| Archive, dead code, and source size | Pending | Account for all 98 archived CPU files, 182 archived SIMD files, 84 `PipelineOp` variants, 313 CPU and 569 SIMD baseline declarations, callers, modes, kernels, and fallbacks. Review active/archive correspondence, remove dead maintained code, and meet the 100-line function / 1,000-line file limits before validation. Keep CPU serial; do not add Rayon. Source-only audit (2026-11-03): active `pool_cpu` has no references to `legacy` or `pool_simd`; `pool_cpu/ops` contains 4,785 Rust files and 165,053 lines. A first-party workspace scan using the exact component exclusions in `RUST_SIZE_POLICY.md` covers 5,517 Rust files and 385,222 physical lines; 26 files are at least 800 lines, none exceed 1,000, and the largest is `pool_cpu/ops/analysis/ops.rs` at 994. Function-size review remains gated until source closure because it uses Clippy; archive correspondence and dead-code review are still pending. |
| Static quality and compilation | Not started; gated on source review | After Stage 5 is complete, run source-map and implementation-structure audits, `make quality`, affected structure/crosswalk checks, `make build-parity`, and available target/feature builds. No unit-test suites for this migration. |
| Full CPU parity | Not started; gated on static quality | Preserve all 17,407 existing CPU-applicable cases (current case-set SHA-256 `8da7b964ad255904d23d15df85f29135f2bf1f9647038330429a9972acde721b`), then expand the parity matrix to operation × exact Mode × each declared CPU Architecture before freezing the final case IDs/hash. Include mode-specific behavior such as I/F and I;16 Brightness, selection/fallback/rejection receipts, output identity, tails, empty inputs, and secondary inputs; do not modify or weaken existing cases. Run the entire expanded CPU-only set after source review and static gates pass. |
| CPU performance | Not started; gated on parity | Compare the frozen 1,032 CPU workloads (ID SHA-256 `639e358082276c924343618d5a3b09c6945de012c91c1a80ac5a8ee2121ccb84`) against compatible pre-refactor evidence. Preserve each workload contract and report each operation/mode/architecture separately; at most one confirmation repeat for questionable pairs. |
| Parallel CPU and later backends | Waiting on CPU completion | Start `pool_parallel` only after CPU source, quality, parity, and performance gates pass. Keep scheduling and evidence separate; GPU work follows later. |

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
the exact-byte binary guards and fusion facts, selected architecture telemetry,
LAB's native three-byte Default binary path, and Default-only `La` binary
behavior. Every Chops mode's Default Offset owner now selects its exact
`DynamicImage` variant and calls a typed buffer helper; the generic
`DynamicImage::offset_with_mode` route was removed from raster transforms.
Accelerated byte and I;16 Offset owners pass typed buffers to shared kernels,
which return the same buffer type without mode matching or output reconstruction.
The checked-u32 guards, endian rules, palette retention, context flow,
fallbacks, and selection/path receipts for Offset have been source-reviewed.
The I;16 architecture guard now agrees with the kernel's 16-byte minimum and
selects same-mode Default for smaller non-empty frames. Source review has
compared the shared scalar and vector Add/Sub, Multiply/Screen, bytewise, LUT,
and logical formulas to the archived CPU/SIMD code; no arithmetic discrepancy
was found. The typed direct wrapper preserves the baseline channel-layout
groups and declares LAB as a CoreOnly extension. LAB binary support is marked
CoreOnly, while the archived
vector guards keep La and LAB secondary operands on Default. Prepared
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
3. Inventory the baseline CPU tree: 98 tracked files and the 84 `PipelineOp`
   variants currently declared in `pillow-rs/src/pipeline.rs`. Record every
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
crosswalk matched all 84 `PipelineOp` variants with no unmapped variants or
unknown `OperationId`s. These changes remain uncompiled and unvalidated; full
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

Complete every family assigned by the structure guide, including all 84
pipeline variants and the direct CPU entry points found in Stage 0.

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

Record this milestone honestly: source review is complete, but compilation,
parity, and performance are not yet proved. Once all items above are complete,
proceed directly to validation without an extra approval request.

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
result. Keep oracle and target processes isolated. This command is documented
from the baseline Makefile; it has not been executed for this plan.

The architecture-aware CPU runner must make its chosen architecture explicit
in evidence, using the prepared plan's actual selection and fallback reason.
Before this stage, add runner-controlled architecture selection at the target
execution boundary and carry it through `ExecutionContext`; do not let mode
implementations read runner environment or infer architecture themselves.
The target receipt must report the context's requested architecture and each
prepared operation's actual implementation/fallback. Keep fixtures and source
oracle cases backend-neutral, and execute target cases once per selected
architecture profile.

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

### ImageOps.pad Python argument ordering

The PyO3 Pad binding computes contain geometry and validates the filter before
converting color or centering inputs. It skips both conversions when contain
already fills the requested dimensions, matching Pillow's direct return of the
resized image. For an actual padded canvas, color conversion still precedes
centering conversion and core color resolution precedes centering access. The
adapter snapshots the input before color and centering callbacks, and filter
callbacks that change dimensions retain Pillow's precomputed contain size for
the current-mode Resize. It validates fill first and indexes only the
coordinate for the padded axis. Exact built-in values use equivalent native
clamp/multiply/round arithmetic; custom numeric values invoke Python's
`min`/`max`, multiplication, and `round` protocols in order. In-range offsets
pass through normalized centering. For mode P, `Image.new` first converts
strings with `ImageColor.getcolor`; it then routes three/four-component
all-integer lists and tuples through a temporary RGB `ImagePalette.getcolor`
call. Pad writes that returned palette index. If the resized source has a
palette, Pad replaces the temporary palette on the result after the lookup; it
does not skip the lookup. The host bridge now calls the package's ImageColor
and ImagePalette adapters in that order and carries the index and temporary
bytes into the native canvas strategy. Integer RGB(A) lists are Pillow-defined
input here. TODO: preserve the eager `Image.new` canvas-allocation precedence
over native fill validation and centering access; preserve signed out-of-gap
custom `round` results and match Pillow's paste-coordinate C-int overflow
behavior; verify callback-driven mode/storage mutation and `getdata` identity,
non-P custom color coercion, and dirty/raw palette failure ordering. Keep these
cases in the frozen CPU parity matrix; no runtime evidence is claimed yet.

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
four-component tuple fills are resolved by that palette's `getcolor` without
passing the source image; this route applies to P and PA.
After palette callbacks it evaluates `image.mode` and captures that Pillow
spelling before size comparisons. Negative-size comparisons run in Python; the
PyO3 boundary then extracts C-int dimensions, translates the captured spelling
to the flat `Mode` enum, and validates exact-mode `ExpandedDimensions`. The
prepared fill and canvas use that typed target, including when a subclass
returns a different valid mode. Non-string fill validation follows those size
checks. The Rust `PreparedExpandFill` is
built against the captured target, while Paste uses the current source wrapper
after size comparisons, preserving Pillow's Image.new-then-paste composition
when callbacks replace or change the source. The canvas uses existing
exact-mode Expand strategies. If a palette snapshot exists, its data is
attached through the wrapper's `putpalette` path before the current source is
pasted, and the final result is materialized at Pillow's eager boundary.

Core derives effective trailing margins from the explicit size and queues the
existing exact-mode Expand strategy without allocating an intermediate
image-sized raster. Remaining source review covers dirty/raw and malformed
palette faults, output metadata, eager `Image.new` allocation timing, and
CoreOnly typed fill behavior. Runtime mode/output/fault parity remains pending.

Both the wrapper preflight and CPU Expand allocation use the declared exact-mode
`ExpandedDimensions` byte count and allow valid zero-sized results. Source
review confirms that the packed mode, native u16, and native f32 loops handle
empty output without staging or converting samples. Palette callbacks run before
the mode property is captured; Python size comparisons run after that; then the
PyO3 boundary resolves the typed target mode and Paste receives the current
source wrapper. Remaining
source gaps are eager `Image.new` allocation-failure timing, dirty/raw and
malformed palette faults, output metadata, CoreOnly typed fill contracts, mode-override output parity, invalid-mode error ordering, and P/PA tuple-fill faults when a subclass suppresses `image.palette`. No
runtime validation has run for this boundary.

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
recorded. Mode admission, same-mode Default fallback, output, and selection
receipts remain for the frozen CPU parity phase; direct/eager caller review is
still part of source closure. No validation has run.

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

Custom-font Draw source continuation (2026-11-03): the facade now applies Pillow's plain-string multiline split, anchor, spacing, alignment, and justification calculations through the custom font's `getbbox`/`getlength` methods. Each positioned line calls `getmask2` with the selected font-mask mode, mode-resolved ink, stroke options, and fractional start, then applies the returned offset. Coverage masks continue through exact-mode Draw Bitmap; RGBA masks set the requested ink alpha and compose through Paste while preserving color channels. Native FreeType strings remain on Rust Text. TODO(pillow-rs#3ptb): consume existing `ImageText.Text` state/positions for custom fonts, support arbitrary `getmask2` args, implement custom-font `textbbox`/`textlength` paths, and cover callback fault ordering plus RGB/RGBA mask composition in CPU parity. Source-only; no build, formatting, lint, test, parity, or benchmark ran. Pillow 12.2 reference: [ImageDraw text mask and fallback path](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageDraw.py#L3190-L3310), [ImageText multiline splitting](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageText.py#L1724-L1940), [mode-specific ink resolution](https://github.com/python-pillow/Pillow/blob/12.2.0/src/_imaging.c#L3092-L3305).

Custom-font Draw binding correction (2026-11-03): RGBA masks now pass the underlying `_rust_image` objects to the PyO3 Paste bridge, matching its `PyImage` inputs instead of passing Python facade wrappers. The `getmask2` fallback is selected only when the method is absent, so an `AttributeError` raised by a custom font propagates through the existing callback path. TODO(pillow-rs#3ptb): custom `ImageText.Text` state, arbitrary `getmask2` arguments, custom metrics, and exact callback/fault and RGB/RGBA composition parity. Source review only; no validation ran.

Autocontrast option source migration (2026-11-03): the typed pipeline and exact L/RGB CPU strategies now carry cutoff pairs, ignored bins, and preserve_tone. Preserve-tone gathers native RGB-to-L statistics and applies one LUT to RGB without changing output mode. Unmasked native L/RGB GPU requests lower the CPU LUT to Eval; masked nonzero cutoffs, ignored bins, and preserve-tone remain a GPU TODO. Python currently coerces cutoff and ignore before mask validation and source histogram materialization, so callback, mask, and source faults can be ordered differently from Pillow. TODO(pillow-rs#3ptb): finish wrapper fault ordering and verify pixel/fault parity by mode and backend. Pillow 12.2 reference: [ImageOps.autocontrast](https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageOps.py#L62-L152).

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

Save/Seek callback borrow staging (2026-11-04): Save extracts format and
compression arguments before PathLike conversion, keeps the image borrow only
for encoding, and releases it before calling a file object's `write` method.
Seek parses its frame value before taking the mutable image borrow. This keeps
Python callbacks outside PyO3 image borrows while preserving the source-level
argument order. Runtime output and fault-order parity remain pending.
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
mode dispatch now preserve these routes. The Python facade loads source and
palette Images in Pillow's order. Arbitrary palette truthiness and host
attribute callbacks remain a code TODO. All 21 CoreOnly mode owners now carry
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
intentionally ignores their values. The Python package currently exposes no
`ImageText.Text` type, and custom-font `textbbox`/`textlength` are still
unimplemented. TODO(pillow-rs#3ptb): add/consume ImageText state and line
positions, implement custom-font metrics, and verify callback faults and
RGB/RGBA composition in frozen CPU parity. Source-only; no build, formatting,
checkstyle, lint, test, parity, benchmark, or coverage ran. Source:
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
normalized RGB carrier. The dedicated LAB branch normalizes CMYK through RGBA
and applies the sRGB-to-LAB ImageCms transform. The CMYK mode strategy no
longer labels those Pillow-supported target pairs CoreOnly, and its palette
path comment now records the wrapper fallback. This changes support
classification only; target output, palette metadata, dither, fault, and
architecture parity remain TODOs for the frozen CPU matrix. Sources: [Pillow
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
continues through the existing ImageCms composition. TODO(pillow-rs#3ptb):
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
