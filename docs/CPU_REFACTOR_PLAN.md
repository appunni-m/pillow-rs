# CPU mode and architecture refactor plan

This plan records the required CPU migration and its verification gates. File
placement is governed only by
[`IMPLEMENTATION_STRUCTURE.md`](IMPLEMENTATION_STRUCTURE.md); this document does
not define a second path grammar. Track implementation work in Kata issue
`pillow-rs#3ptb` and related behavior findings in `pillow-rs#brg0`.

## Goal

Make supported behavior visible for every operation and Pillow mode, then select
an implementation by composing the mode strategy with the architecture strategy
from one injected execution context. Keep the Pillow mode identity distinct
from pixel storage and codec metadata.

`pillow-rs` owns a payload-free mode enum on its image wrapper and lazy pipeline
state. The enum represents these 25 identities:

`One`, `ImageLuma8`, `ImageLumaA8`, `ImageLaPremultiplied8`, `ImagePalette8`,
`ImagePaletteAlpha8`, `ImageRgb8`, `ImageRgba8`, `ImageRgbPremultiplied8`,
`ImageRgbx8`, `ImageCmyk8`, `ImageYCbCr8`, `ImageLab8`, `ImageHsv8`, `ImageI32`,
`ImageF32`, `ImageI16`, `ImageI16L`, `ImageI16B`, `ImageI16N`, `ImageLumaA16`,
`ImageRgb16`, `ImageRgba16`, `ImageRgb32F`, and `ImageRgba32F`.

Raster storage remains its own enum; multiple logical modes may share one
storage representation. `image-slash-star::ImageMode` remains codec sample
metadata and is translated at Pillow-owned decode/encode boundaries. Python
parses Pillow mode strings into the core enum before calling typed Rust APIs;
string formatting remains at host/API boundaries.

## Exhaustive change inventory

1. **Mode ownership and typed boundaries**
   - Add the payload-free, pillow-rs-owned mode enum and map all 25 identities
     to their current storage and public Pillow spelling.
   - Carry it through loaded images, lazy pipeline nodes, mode transitions,
     materialization, mutations, and wrapper-facing metadata.
   - Keep codec mode metadata separate; convert explicitly in decode and encode
     adapters.
   - Parse Python strings in the binding and pass the typed enum to Rust.

2. **Operation support declarations and strategies**
   - Give each operation an exhaustive top-level support declaration covering
     all 25 modes, including supported/unsupported status, storage and output
     mode, parameter guards, build/architecture requirements, and exact fallback
     or rejection behavior.
   - Use the same declaration for support reporting, admission, and prepared
     implementation selection.
   - Compose the selected mode strategy with the injected architecture strategy.
     Architecture fallback must preserve the same mode and use its exact default
     implementation.
   - Keep shared helpers outside mode strategies only when they are truly reused
     under the same contract. Do not infer logical mode from storage.

3. **Execution-context injection**
   - Construct the immutable backend/architecture context at the routing
     boundary and pass it through planning, admission, registry dispatch,
     pipeline and eager execution, fusion, nested operations, fallbacks, and
     family orchestration.
   - Remove per-operation context reconstruction and context-dropping registry
     fallback paths.
   - Keep mode as wrapper-owned typed state passed at the family boundary.

4. **CPU family migration**
   - Keep `pool_cpu` serial. Rayon scheduling and parallel-only behavior belong
     to `pool_parallel` and are out of this CPU checkpoint.
   - Preserve the existing legacy archives as inert references only. Confirm
     their provenance against the requested pre-`3f9dfd403` source; never import
     or execute archived modules from maintained code.
   - Rebuild every CPU family in the approved family/mode/architecture shape
     from `IMPLEMENTATION_STRUCTURE.md`, using archived implementations as the
     behavior reference. Split only where a mode or architecture selects a
     genuinely distinct strategy.
   - Preserve exact algorithms, arithmetic order, native sample representation,
     errors, output modes, specialized hot loops, and existing public routing.
   - Never cast between logical modes just to reuse code or reduce duplication.
     Implement each supported mode against its native samples and channel
     layout; share only helpers with an identical native contract. Explicit
     mode-changing operations retain their specified conversion semantics.
   - Pillow parity is the compatibility baseline, not a cap on core support.
     Mark every intentional extension with a nearby `NO_PILLOW_SUPPORT`
     comment and keep its result separate from Pillow-defined parity.
   - Remove unreachable or redundant code only after source-level reachability
     review. Add an actionable `TODO(pillow-rs#3ptb)` at each owner whenever a
     path remains pending; keep the issue ledger current.

5. **Source-only completion review**
   - Review every public operation and direct entry point against the support
     declarations and its archived behavior reference.
   - Account for all helpers, specialized kernels, callers, nested paths,
     transitions, and same-mode fallbacks. Confirm no archived code is imported.
   - Keep every first-party Rust file at or below 1,000 physical lines and every
     function at or below 100 Clippy-counted body lines.
   - Do not run build, formatting, checkstyle, lint, tests, parity, or benchmark
     commands until the complete CPU implementation and source review are done.

6. **Verification after the source review is complete**
   - Run the repository's planned static-quality and compilation gates.
   - Run the complete CPU parity denominator only, without modifying tests or
     running Parallel CPU parity.
   - Run the CPU benchmark comparison under the documented quiet conditions;
     accept at most 1–5% deviation and use no more than the one permitted
     confirmation repeat.
   - Record operation-level results, failures, unresolved TODOs, and any
     evidence limitations in Kata and the final report. Do not close the work
     until the required verification passes.

## Order after CPU

Complete and verify CPU first. Then migrate the separate `pool_parallel`
backend, reusing CPU algorithms through composition and keeping Rayon scheduling
out of CPU. Continue with other backend work only after those checkpoints.

## Current open paths

The 96 files under `pool_cpu/legacy/ops/` were compared with the operation tree
at the parent of `3f9dfd403`. Every file maps to that committed source; 85 are
byte-identical, while the other 11 differ only in relative `#[path]` or
`include_bytes!` paths needed after archiving. No non-path source differences
were found. The archive remains an inert reference and was not changed.

The payload-free core `Mode` enum now exists in `pillow-rs/src/image/mode.rs`.
`LoadedData` computes it once, lazy pipeline nodes carry it when known, and
`Image::mode_selector()` exposes it. CPU registry and most operation entry
points still pass mode strings, and constructors are not yet typed. Eval now
receives a `Mode` strategy and maps native byte samples without widening or
inferring channel count from storage; the registry temporarily parses its
existing mode string at the callback. Reduce now declares its exhaustive
typed mode strategy in the family folder and validates every mode/carrier pair.
The four `I;16*` identities use native `u16` averaging; internal LA16, RGB16,
RGBA16, RGB32F, and RGBA32F use typed native sample reducers and carry a nearby
`NO_PILLOW_SUPPORT` marker. Grayscale also declares exhaustive Mode dispatch
and carrier checks. Outstanding TODOs cover the CPU registry's context-dropping
fallback, the remaining string-based registry mode handoff, native mode paths
for Composite, Draw, Pad, and Thumbnail, plus the remaining per-family support
declarations. Enhance now lives under `pool_cpu/ops/enhance/`; its four
operation entry points take `Mode`, and the family declares exhaustive mode
strategies with carrier validation. Brightness uses native byte, u16, i32, and
f32 sample kernels; ColorSaturation and Sharpness use native byte, u16, i32,
and f32 samples with direct mode-specific channel rules. Core-only One/P/PA and
internal high-depth paths carry nearby `NO_PILLOW_SUPPORT` markers. Contrast
still uses the conversion-defined fallback for noncanonical modes and has an
owner-local TODO. The registry currently translates its compatibility mode
string when calling Enhance; remove that bridge when the registry carries
cached `Mode`. Reduce's compatibility string and untagged storage-derived
alpha path remain TODOs until all callers forward cached Mode.
Expand now lives in its own family folder. `expand/ops.rs` declares all 25
mode/carrier pairs and orchestrates execution; serial native byte, u16, and f32
sample-copy kernels live in `expand/kernels/default.rs`. Its internal
high-depth paths carry nearby `NO_PILLOW_SUPPORT` markers.
ImageOps Autocontrast, Invert, Posterize,
Solarize, and the palette materializer now include explicit native P paths;
Solarize also has a native One extension. Each intentional extension is marked
`NO_PILLOW_SUPPORT`. P prefixes execute on index bytes before a color-dependent
suffix expands through the palette. Telemetry aggregation across that split and
manually constructed flattened P-to-PA/L transitions remain TODOs. I/F filter
entries now reject mismatched storage instead of casting it to the four-byte
carrier, and YCbCr grayscale requires its native byte triples. These are
migration markers, not completion claims. Resolve or update each TODO as its
path is migrated.

Resize is now a folder family at `pool_cpu/ops/resize/`. Its `ops.rs` has an
exhaustive contract for all 25 modes, including native carrier, preserved
output mode, I/F/One strategy, and scalar-FMA eligibility with the context's
default fallback. The distinct `one/`, `i/`, and `f/` mode strategies own their
default kernels; I and F put x86 scalar-FMA entry kernels under `kernels/fma.rs`.
The archived numeric resampling arithmetic and boxed I passes were relocated
without changing their arithmetic order. The registry still translates its
compatibility string to `Mode`, direct registry callbacks retain a string
adapter, and support reporting/admission do not yet consume the same contract.
These paths remain TODOs under `pillow-rs#3ptb`; this Resize migration is
incomplete and unvalidated.
