# Implementation structure

Status: required target structure, not a statement that migration is complete.
The restart baseline is `3f9dfd4036caa061eabcf2255c9f34a745df9c7e`.
The execution order and completion gates are in the
[CPU refactor plan](CPU_REFACTOR_PLAN.md).

This file is the placement authority for this refactor. Existing layouts and
older architecture descriptions are migration inputs, not alternate designs.
Do not restructure unrelated parts of the repository during the CPU phase.

Every operation must expose an explicit, typed support contract at its entry
point. Mode and architecture strategies implement that contract. Folder
placement separates implementation ownership; support is declared in code and
is inspectable without reading folders or running an operation. Section 3
defines the required support and strategy interfaces. These are target design
requirements; their presence here does not mean they are implemented yet.

## 1. Backend, architecture, and mode

- Backend selects the executor: CPU, later Parallel CPU, or GPU.
- Architecture selects a CPU implementation: `default`, `neon`, `sse2`,
  `sse3`, `ssse3`, `avx`, `avx2`, `wasm_simd128`, or another specifically
  implemented instruction set.
- SIMD is folded into CPU. Maintained vector implementations belong to
  `pool_cpu`, with architecture expressed by context. There is no independent
  SIMD executor or maintained `pool_simd` implementation after the CPU phase.
- Mode comes from the Pillow-owned `raster::DynamicImage` value. Backend and
  architecture belong to execution context, not to image pixel metadata.
- Parallel scheduling belongs to `pool_parallel` after the CPU completion gate.
  CPU remains serial, including when the Cargo `parallel` feature is enabled.

If existing public APIs accept `simd`, translate that request once at the
routing boundary to CPU with an eligible architecture. Preserve public API
compatibility through this narrow adapter; do not retain a second SIMD registry,
pool, or execution path. Report the actual backend and architecture separately.

## 2. Mode-bearing raster

`pillow-rs/src/raster/dynamic.rs` owns the materialized image enum.
Its flat variants carry the exact mode identity and the appropriate buffer.
The enum discriminant is the stored identity; do not add a separately stored
`Mode` plus `PixelStorage` pair to each materialized image.

For example, `ImageLuma8(buffer)`, `ImagePalette8(buffer)`,
`ImageRgba8(buffer)`, and `ImageCmyk8(buffer)`
must remain distinct even when their payload buffers have identical physical
layouts.

Construction, decode adaptation, and explicit conversion choose the variant
once. Clone, crop, transpose, and other mode-preserving operations preserve the
identity. Conversion, quantization, band extraction, alpha insertion, and any
other mode-changing operation construct the exact resulting variant.

The materialized enum and the payload-free `raster::Mode` selector must have
these same 25 identities. `DynamicImage::mode()` reads the existing variant
tag and returns `Mode`; it does not inspect samples or add another stored tag.
Use the same variant names for both enums, with buffers only on `DynamicImage`.

| Rust variant | Pillow spelling | Required mode folder |
| --- | --- | --- |
| `One` | `1` | `one` |
| `ImageLuma8` | `L` | `luma8` |
| `ImageLumaA8` | `LA` | `luma_a8` |
| `ImageLaPremultiplied8` | `La` | `la_premultiplied8` |
| `ImagePalette8` | `P` | `palette8` |
| `ImagePaletteAlpha8` | `PA` | `palette_alpha8` |
| `ImageRgb8` | `RGB` | `rgb8` |
| `ImageRgba8` | `RGBA` | `rgba8` |
| `ImageRgbPremultiplied8` | `RGBa` | `rgb_premultiplied8` |
| `ImageRgbx8` | `RGBX` | `rgbx8` |
| `ImageCmyk8` | `CMYK` | `cmyk8` |
| `ImageYCbCr8` | `YCbCr` | `ycbcr8` |
| `ImageLab8` | `LAB` | `lab8` |
| `ImageHsv8` | `HSV` | `hsv8` |
| `ImageI32` | `I` | `i32` |
| `ImageF32` | `F` | `f32` |
| `ImageI16` | `I;16` | `i16` |
| `ImageI16L` | `I;16L` | `i16l` |
| `ImageI16B` | `I;16B` | `i16b` |
| `ImageI16N` | `I;16N` | `i16n` |
| `ImageLumaA16` | Internal representation | `luma_a16` |
| `ImageRgb16` | Internal representation | `rgb16` |
| `ImageRgba16` | Internal representation | `rgba16` |
| `ImageRgb32F` | Internal representation | `rgb32f` |
| `ImageRgba32F` | Internal representation | `rgba32f` |

Case-sensitive modes must not collide after folder normalization. In particular,
`LA` differs from `La`, and `RGBA` differs from `RGBa`. Keep the four `I;16*`
identities distinct even when numeric processing shares a helper.

The five internal high-depth identities participate in Rust strategy selection
and support reporting. They do not introduce new accepted Pillow strings.
Do not narrow, normalize, or discard them to fit the new enum.

Keep existing sample representation and arithmetic during the migration.
In particular, replacing the existing raw four-byte carriers for `I` and `F`
with a different numeric representation is not required by this refactor.
Preserve byte-order behavior, palette indices, palette alpha, LAB conventions,
and straight versus premultiplied alpha semantics.

`raster/mode.rs` owns the payload-free selector used by Rust construction,
conversion parameters, lazy output metadata, and support queries. It must not
become a second independently mutable tag on a materialized image. Read the
image's enum tag rather than inferring identity from channels, `ColorType`,
buffer contents, or string overrides.

Python accepts Pillow's case-sensitive strings and converts them to the typed
selector before calling core operation APIs. Other host bindings or explicit
I/O adapters own their corresponding parsing and formatting boundaries. Core
operation parameters, support declarations, and CPU strategies use enums;
they must not accept string-based modes. Preserve existing invalid-string
errors at the public boundary. Adapt decoded image-slash-star metadata once
at the decode boundary. Keep codec objects and Pillow raster objects separate;
do not add Pillow execution metadata to image-slash-star types.

### 2.1. Mode ownership and codec boundaries

Decision: `pillow-rs` owns the canonical operation-facing `raster::Mode`.
Rust callers use that enum directly; Python translates its Pillow strings to
that same enum before delegation. A codec's pixel-format descriptor is not a
second authority for operation mode selection.

`pillow-rs` owns `raster::Mode`, the mode-bearing `raster::DynamicImage`,
mode transitions, and the mode contracts used by operation strategies.
These are Pillow semantics: identical storage can represent distinct modes
such as luma and palette indices, or straight alpha, premultiplied alpha,
padding, and CMYK. A generic pixel-buffer type cannot establish that identity.

This identity is independent of backend. CPU, the later Parallel CPU backend,
and GPU consume the same Pillow-owned enum; they must not define backend-local
copies of Mode. The enum names every representable identity, not the modes
supported by every operation. Each operation declares that narrower contract.

image-slash-star owns its codecs, encoded/decoded image types, buffers, and
format metadata. Its own mode or pixel-format descriptors retain their codec
role; do not alias or re-export one as the Pillow-owned `raster::Mode`, add
Pillow strategy metadata to its types, or require that repository to implement
this refactor's mode model.

The ownership boundary is explicit:

| Concern | Owner | Required boundary |
| --- | --- | --- |
| Pillow mode identity and transitions | `pillow-rs` raster and operation contracts | Flat `DynamicImage` variants and the matching typed `Mode` selector |
| Pixel buffers, codecs, and format metadata | image-slash-star | Keep its types independent; translate through Pillow-owned adapters |
| Pillow string spellings | Python binding and explicit I/O boundaries | Parse once before typed Rust operation calls; format only for external output |
| Backend and architecture selection | `pillow-rs` compute context | Inject the resolved policy into operation preparation and execution |
| Supported mode/architecture combinations | Each operation | Declare them at the operation entry point and bind them to executable strategies |

Pillow-owned and codec-owned types with similar names remain distinct types.
Reusing a buffer does not transfer ownership of Pillow semantics to the codec
repository. A matching physical layout is insufficient to select or change a
Pillow mode; the adapter or explicit conversion must establish that identity.

The Pillow-owned decode adapter performs this translation once:

```text
image-slash-star decoded image and format metadata
    -> Pillow decode adapter
        -> pillow-rs raster::DynamicImage with exact mode identity
            -> operation mode strategy
```

Use decoded metadata and the existing explicit construction/decode policy to
choose the variant, preserving samples, palettes, alpha, and byte order.
Preserve the existing rejection contract when the input cannot be represented;
do not defer ambiguous mode recovery to CPU implementations. Encoding performs
the corresponding translation at the codec boundary, preserving existing
explicit conversion requirements and errors.

The Python binding owns Pillow string parsing/formatting and passes the typed
Pillow selector to Rust. Core operations use the Pillow-owned image identity
and typed parameters. Backend and Architecture remain execution-context
dependencies; neither belongs to codec metadata or the image's mode identity.

Public Rust APIs that accept a mode use this same Pillow-owned enum. Python's
string spelling is a boundary representation of that identity, not another
mode model. The operation reads the materialized image's variant; it does not
reconstruct a mode from the upstream buffer type, channel count, or spelling.

Use this same enum type in Rust operation arguments, operation support
descriptors, and prepared mode strategies. A support query must classify the
identity that execution will consume, without translating through a string or
an image-slash-star descriptor. Review these API signatures together with the
Python parsing and codec adapters to verify the ownership boundary.

## 3. Injected execution context

`pillow-rs/src/compute/execution_context.rs` owns the backend and architecture
context and its construction policy. Construct it once when a pipeline or eager
operation is routed. Pass the selected context through registry dispatch,
admission, batch execution, fusion, fallbacks, and nested operations.

Backend may be explicitly requested or resolved from existing runtime policy.
Explicit selection takes precedence over automatic routing according to the
existing public contract. Leaf functions must not re-read global backend state
or construct a fresh default context.

Determine available architecture implementations from compile-time target and
feature configuration. A statically guaranteed target can select its
architecture without runtime detection. A portable binary that includes
optional instructions may detect capabilities once when initializing the
architecture policy, then inject the validated selection. Static analysis
cannot establish the instruction support of every machine running a portable
binary. Never execute AVX, AVX2, SSSE3, or another optional instruction set
without its required capability guarantee.

`Architecture::Default` selects the ordinary serial implementation. SSE3 and
SSSE3 are different instruction sets and must not share a misleading label.
An implementation requiring additional capabilities, such as FMA, must declare
them precisely. Do not assume a higher-numbered architecture can call every
lower-level kernel without checking the actual requirements centrally.

Context-aware selection matches the operation, image variant, and architecture
outside pixel loops. A validated specialization may also have existing shape,
alignment, or parameter requirements. If it cannot handle a supported image,
call the matching mode's default implementation through a bounded, explicit
fallback. Preserve the image identity and operation arguments. Do not recursively
reroute through global backend selection.

Avoid parallel booleans, repeated feature detection, string mode parameters,
and per-pixel dispatch. Use direct calls to typed implementations after routing.
Pass helpers only the context or prepared dependencies they actually need;
helpers that need neither must not manufacture a selection decision.

### 3.1. Operation-level support is the front door

Every operation has a typed `OperationSupport` descriptor, available through
its operation identity and through the corresponding `PipelineOp` value.
Direct operations outside `PipelineOp` expose the same contract. A family
containing several operations must report each operation independently;
one sibling's implementation must never imply support for another sibling.

The descriptor at the operation's entry point must make these facts visible:

| Contract field | Required information |
| --- | --- |
| Operation | Typed identity, including variants whose parameter contracts differ |
| Modes | Exhaustive classification of all 25 source identities, with rejection reasons |
| Other inputs | Destination, secondary-image, mask, band, palette, and alpha constraints where relevant |
| Result | Preserved or explicitly derived output mode and metadata requirements |
| Backend and architecture | Registered strategies, exact instruction requirements, and build availability |
| Request guards | Existing shape, alignment, filter, parameter, or content conditions on a strategy |
| Fallback | Explicit same-mode Default strategy and the conditions selecting it |
| Implementation | Stable strategy identity linking the declaration to its executable implementation |

Supported combinations are relations between inputs. Listing two modes
individually does not imply every binary combination or conversion between
them is supported. Reject invalid requests with the existing public error.
Every new `Mode` must require explicit classification by each operation;
use exhaustive, explicitly named supported and rejected arms. Do not infer
support from storage layout.

The top-level view must include rejected identities even when they have no
implementation folder. For each accepted identity, show the output contract,
Default implementation, declared specializations, and their availability or
guards. Keep declared support, eligibility for a particular request, actual
selection, and verification evidence distinct; a single supported boolean
cannot express this contract.

The following illustrates the planned interface; these APIs are not yet
implemented or validated:

```rust
let support = OperationId::Resize.support(); // Full operation contract.
let support = op.support();                 // Same descriptor via PipelineOp.
let assessment = op.assess(&input_metadata, &ctx);
let prepared = op.prepare(&inputs, &ctx)?;
let selection = prepared.selection();       // Actual route and fallback reason.
let output = prepared.execute()?;
```

`support()` describes declared strategies and availability in the current
build without executing the operation. `assess()` applies the same eligibility
rules to a particular request. Its typed outcome is ready with a selection,
rejected with a reason, or deferred with the unresolved guards. Unmaterialized
inputs or unavailable content facts must remain deferred until preparation
can establish them; a mode-only query cannot promise request admission.

`prepare()` completes validation, injects dependencies, and returns an
operation-specific executable plan or the existing error. The plan retains
the validated inputs and actual selection. Admission and execution consume
this decision; execution must not independently choose a different strategy.
Borrow or move inputs and prepared dependencies according to existing ownership;
do not clone pixel buffers merely to carry strategy or context. Avoid repeating
expensive preparation solely for a support query.

Make requested and selected backend/architecture, implementation identity,
and fallback reason inspectable on the plan and execution receipt. Distinguish
an absent specialization, a feature-disabled build, unavailable hardware,
and a failed eligibility guard. A declared strategy's parity/performance
evidence is a separate status; implementation registration does not prove it.

### 3.2. Compose mode and architecture strategies

Use the Strategy pattern at both levels, with explicit composition:

```text
Operation contract + request + injected ExecutionContext
    -> exact mode strategy
        -> eligible architecture strategy, or declared Default fallback
            -> prepared typed execution plan
                -> original algorithm and typed pixel loops
```

A mode strategy owns the operation's semantics for one exact mode: input
contracts, output identity, and prepared dependencies such as palettes,
masks, coefficients, or border behavior. It composes an architecture strategy
implementing those semantics. Distinct identities such as palette indices and
luma remain distinct mode strategies even when a proven common helper is reused.

Core operation support may be a superset of Pillow. Preserve Pillow-defined
behavior exactly, and keep any additional mode or behavior as an explicit
native-mode strategy with its own declared input/output contract. Put a nearby
`// NO_PILLOW_SUPPORT: <reason>` comment at the implementation of each such
extension so it is not mistaken for Pillow parity. Report core extensions
separately from Pillow-defined parity evidence; the extension must not rely on
casting samples or converting to another mode merely to reuse an algorithm.

An architecture strategy owns the concrete implementation for that mode and
its exact capability/eligibility requirements. It receives typed buffers and
prepared dependencies. It must not infer the mode, choose the backend, probe
hardware, or introduce parallel scheduling inside its algorithm.

Represent runtime choices with operation-specific concrete strategy types and
closed enums. For example, a `PreparedResize` enum selects a concrete mode
strategy, whose architecture enum selects Default, Neon, or an implemented
AVX strategy. Match once at each selection/execution boundary, outside the
pixel loops, then call concrete functions directly. An operation-specific
trait may express a shared contract where useful; the Strategy pattern does
not require heap allocation or trait-object dispatch. Avoid a universal
untyped strategy that moves mode branching back into helpers.

Context supplies the already validated backend and architecture policy;
the operation composes its eligible strategy from that context and its inputs.
Keep the context immutable. Record a narrower operation-specific selection
on the plan rather than rebuilding context or repeating feature detection.
Nested operations use the injected context and their own declared strategies.

Every supported CPU operation/mode has an executable serial Default strategy.
Fallback is bounded and explicit: a missing or ineligible specialization may
select that same mode's Default strategy. It must preserve arguments, output
identity, and errors. Invalid requests are rejected; execution failures are
not a reason to silently retry another implementation. Do not add fallback
mode conversions or recursive backend rerouting.

### 3.3. One declaration drives support and execution

Place each operation's support declaration beside its entry point in family
`ops.rs`, with its exhaustive mode classification visible there. Each mode's
`ops.rs` declares the concrete architecture strategies it composes, including
requirements, guards, constructor, and Default fallback. The operation-level
descriptor exposes these linked declarations so a caller can inspect the
entire operation without traversing its implementation directories.

Use one typed registration/declaration to bind support to executable strategy
constructors and selection arms. A small compile-time declaration macro may
generate repetitive exhaustive matches and descriptor entries where needed;
do not keep an independent hand-maintained support boolean/table alongside
different dispatch logic. Keep registration and introspection outside pixel
loops and preserve direct calls, inlining, and allocation boundaries.

The compute registry aggregates operation contracts and delegates admission
and preparation to their owners. Shared support types belong to
`compute/registry/support.rs`, exported deliberately from the registry facade.
The registry must not reimplement per-mode eligibility or architecture policy.
Keep `mod.rs` files limited to declarations and exports.

Derive any top-level support matrix from these operation descriptors. Show
rejected modes and unavailable architectures with reasons, as well as usable
specializations and Default fallbacks. Folder existence, a non-null registry
function pointer, and successful compilation are insufficient support claims.

Acceptance is reviewed from the operation's entry point. A caller must be able
to enumerate all 25 identities, inspect their input/output contracts, and see
declared architectures, build availability, and fallback/rejection reasons
without opening implementation folders or materializing pixels. Request facts
that require pixels remain explicit deferred guards. For a prepared request,
the same interface must expose the selected implementation and fallback reason;
execution must consume that prepared selection. This is a required API contract
for the migration, not a claim that every operation already implements it.
The descriptor must also distinguish Pillow-defined support from intentional
core-only extensions, including extensions unavailable through Pillow's
string-mode API.

## 4. Exact CPU placement

The implementation path is:

```text
pillow-rs/src/compute/pool_cpu/ops/<family>/<mode>/<arch>/<files>
```

`<family>` is the owner listed below, `<mode>` uses the exact mapping above,
and `<arch>` is `default` or the actual instruction set implemented there.
The `default` folder is required for the new architecture-based layout; this
supersedes the earlier design in which serial CPU had no architecture level.
Do not insert an additional operation or `kernels` directory into this path.
Different operations in one family use appropriately named implementation
files within the selected mode and architecture.

Example structure; architecture folders exist only for real implementations:

```text
pool_cpu/
├── mod.rs
├── executor.rs
├── batch_execution.rs
├── helpers/
│   └── <support category>/
│       ├── mod.rs
│       └── <specific support>.rs
├── ops/
│   ├── mod.rs
│   └── resize/
│       ├── mod.rs
│       ├── ops.rs
│       ├── helpers/
│       │   ├── mod.rs
│       │   └── coefficients.rs
│       └── rgba8/
│           ├── mod.rs
│           ├── ops.rs
│           ├── default/
│           │   ├── mod.rs
│           │   └── resize_impl.rs
│           └── neon/
│               ├── mod.rs
│               └── resize_impl.rs
└── legacy/
    └── <preserved original source tree>
```

- Every `mod.rs` created or migrated for this CPU structure contains only module
  declarations and deliberate exports. The pool facade lives in `executor.rs`;
  `mod.rs` re-exports it. Do not put algorithms or orchestration in `mod.rs`,
  or add a sibling `<family>.rs`.
- Family `ops.rs` exposes each operation's support contract and composes the
  exact mode strategy. Mode `ops.rs` exposes and selects its architecture
  strategies. Keep their functions small and limited to orchestration;
  inline direct forwarding when appropriate. Follow Section 3's shared
  declaration contract rather than introducing independent support checks.
- Implementation files own the original algorithm and typed loops. Their names
  describe the operation, such as `resize_impl.rs` or `color3dlut_impl.rs`.
- Create mode folders only for actual supported implementations. Handle rejected
  modes explicitly at the family boundary using the existing error contract.
  Do not create twenty empty mode folders for every family.
- Create architecture folders only for real kernels. Missing specialization
  means an explicit call to that mode's default implementation.
- Architecture-specific compile gates and target-feature annotations remain
  necessary for safe compilation. They must not substitute for context-driven
  selection or cause repeated runtime capability probing in leaf code.

## 5. Family ownership

This table assigns the baseline's 84 `PipelineOp` variants to target owners.
It is a migration map, not a replacement for the source enum or parity manifest.
Public CPU operations that bypass `PipelineOp` must also be inventoried and
migrated into their corresponding owner.

| Family | Pipeline operation ownership |
| --- | --- |
| `resize` | `Resize`, `ResizeBoxed` |
| `crop` | `Crop` |
| `rotate` | `Rotate` |
| `transpose` | `Transpose`, `Flip`, `Mirror` |
| `thumbnail` | `Thumbnail` |
| `reduce` | `Reduce` |
| `color` | `Convert`, `ConvertLab`, `RemapPalette` |
| `color3dlut` | `Color3DLut` |
| `filter` | `Filter3x3`, `Filter5x5`, `GaussianBlur`, `BoxBlur`, `BoxBlurXY`, `MedianFilter`, `MaxFilter`, `MinFilter`, `RankFilter` |
| `imageops` | `Autocontrast`, `Equalize`, `EqualizeMasked`, `Invert`, `Posterize`, `Solarize`, `Colorize`, `Expand`, `CropBorder` |
| `geometry` | `Contain`, `Cover`, `Fit`, `Pad`, `Scale` |
| `grayscale` | `Grayscale` |
| `chops` | `Add`, `Subtract`, `Multiply`, `Screen`, `Darker`, `Lighter`, `Difference`, `Overlay`, `HardLight`, `SoftLight`, `AddModulo`, `SubtractModulo`, `LogicalAnd`, `LogicalOr`, `LogicalXor`, `Constant`, `Offset`, `Duplicate`, `InvertChops` |
| `enhance` | `Brightness`, `Contrast`, `ColorSaturation`, `Sharpness` |
| `effects` | `EffectSpread`, `EffectNoise` |
| `paste` | `Paste` |
| `composite` | `AlphaComposite`, `Merge`, `BlendModule`, `CompositeModule` |
| `eval` | `Eval` |
| `transform` | `Transform` |
| `put` | `PutPixel`, `PutData`, `PutAlpha`, `PutAlphaData`, `ExtractBand` |
| `draw` | `DrawLine`, `DrawRectangle`, `DrawRoundedRect`, `DrawEllipse`, `DrawCircle`, `DrawPolygon`, `DrawArc`, `DrawChord`, `DrawPieslice`, `DrawPoint` |

## 6. Shared code and composition

Use the narrowest scope that owns a real shared contract:

- One implementation: ordinary helper files beside that implementation.
- One mode, several architectures: `<family>/<mode>/helpers/`.
- One family, several modes: `<family>/helpers/`.
- Several CPU families: `pool_cpu/helpers/<support category>/<support>.rs`.
- Several backends: `compute/helpers/<support category>/<support>.rs` for
  execution support, or `raster/` for pixel-model support.

Each helper directory has a simple `mod.rs`. Create a directory only for a real
group of support code. Shared helpers must not hide mode inference, backend
selection, architecture detection, or parallel scheduling.

Reuse a CPU kernel directly when the algorithm and semantics are identical.
Do not copy an identical implementation into each mode or architecture merely
to fill the tree. Keep genuinely distinct loops specialized and preserve
inlining and existing arithmetic order. Do not add trait-object dispatch to
pixel loops to reduce source duplication.

Do not cast, convert, or retag an image into a different mode merely to reuse
another mode's implementation. A mode strategy must process its own native
buffer and return the declared output identity. Share a helper only when it
operates directly on the native sample representation and preserves the exact
mode's semantics; do not round-trip through an intermediate raster. A
conversion that is part of the operation's defined Pillow behavior is allowed,
but it must be explicit in that operation's contract and output-mode rule.

Inject palettes, masks, prepared coefficients, and other operation dependencies
at the appropriate boundary. Secondary images carry their own exact modes;
validate required relationships once. Operations must not silently convert all
inputs to RGBA to share a kernel.

## 7. Legacy and other backends

Before rebuilding, move all existing `pool_cpu` source into
`pool_cpu/legacy/`, preserving relative paths and contents from the restart
baseline. Recreate only the maintained facade and new structure outside it.
When folding SIMD, preserve its original source under
`pool_cpu/legacy/pool_simd/` before retiring the old pool.

Legacy is reference-only: no `mod legacy`, `#[path]`, `include!`, generated
module, build-script wiring, or runtime import may activate it. Do not use
`legacy_2` or the external `pillow-rs-bad` checkout as a runtime dependency.
The external checkout is optional historical reference, not the baseline.
Record original-to-new ownership and preserve backups without editing them.

Remove dead code from maintained modules after accounting for feature-specific
and architecture-specific reachability. Keep uncertain findings reviewable.
Inventory tools may identify legacy as an archive; they must continue checking
all maintained implementation files and must not conceal size violations.

Parallel CPU later follows the same mode/architecture organization under
`pool_parallel`. It owns Rayon and scheduling, injects its backend context, and
composes reusable serial CPU kernels. GPU algorithms, shader organization,
CUDA, and TileLang work are subsequent work, outside this CPU migration.
Mechanical compatibility edits needed for shared type changes must be recorded;
they are not evidence that another backend has been validated.

## 8. Enforcement

Keep first-party Rust files at or below 1,000 physical lines and functions at
or below 100 Clippy-counted body lines. Preserve assertions, fixture identities,
public error behavior, exact arithmetic, and existing performance contracts.

Update path-classification rules, source maps, and relevant contributor docs
to describe this structure. Do not weaken checks or create blanket exceptions
to make an old layout pass. Other maintained files keep their existing owners;
this CPU plan does not authorize unrelated repository-wide file moves.

No build, test, formatter, checkstyle, lint, parity, benchmark, or coverage
campaign runs during implementation. Complete the code and the source review
first. The ordered validation gates in the refactor plan govern what runs next.
