# Architecture and source ownership

pillow-rs has one Rust image implementation and language bindings. The
[repository map](REPO_MAP.md) locates the maintained source and generators.
The [implementation structure guide](IMPLEMENTATION_STRUCTURE.md) defines the
shared operation-family layout used to trace code across core, backends,
bindings, parity inputs, and performance evidence. The generated
[operation/source crosswalk](IMPLEMENTATION_STRUCTURE.md)
helps locate mapped and review-only code; its current reachability limits and
investigation findings are recorded in the
[unreachable-code audit ledger](UNREACHABLE_CODE_AUDIT.md).

| Component | Owns | Public entry |
| --- | --- | --- |
| `pillow-rs/` | Image modes, pixel buffers, transforms, drawing, operations, pipelines | Rust `pillow_rs` |
| `pillow-rs-py/` | Python conversion and delegation | Python `PIL` |
| `pillow-rs-js/` | JavaScript conversion, WASM lifetime, runtime initialization | npm `pillow-rs` |
| image-slash-star dependency | Image codec parsing, encoding, and format semantics | Byte-buffer codec API |
| fontdone dependency | Font parsing, metrics, hinting, outlines, and rasterization | Rust font API |

Keep algorithms in core. Binding code converts host values and delegates;
it must not implement an alternate image algorithm. Core runtime logic is
Rust. Native Pillow, FreeType, and codec libraries are test oracles, never
runtime substitutes for missing implementation.

Host bindings own inspection and conversion of host objects. They pass
host-neutral Rust values to core for shared Pillow semantics, validation, and
algorithms. Shared input contracts may retain enough type and value information
to reproduce Pillow-compatible errors; they must not expose PyO3, Python, or
JavaScript runtime types. Keep Python and JavaScript extraction and exception
translation in their respective bindings. Existing Python-prefixed Rust paths
for shared inputs are hidden compatibility aliases; remove them only with an
explicit breaking-release decision.

## Image and backend boundaries

An image retains its native mode and layout. Drawing writes the native pixel
format; converting every input to RGBA changes observable behavior and cost.
Mode conversion is an explicit operation.

CPU, SIMD, and GPU execution share public semantics, with different capability
and routing limits. A backend request can include host work or fallback.
Execution receipts record requested and actual routing plus terminal completion.
A registration, shader compilation, or GPU request is not proof of native
arithmetic or source-line coverage.

The former ImageBatch executor was an unshipped prototype, not a public package
API. The current [explicit GPU executor](IMAGE_BATCHING.md) reuses the existing
`PipelineOp` enum and lazy image graph. Core owns native encoding and
credit-bearing output leases; the Python Results iterator drives bounded keyed
admission and delivery.

## Safety and error conventions

Use checked dimensions and the centralized buffer helpers before allocation.
Use the maintained parallel helpers and canonical conversion paths instead of
duplicating buffer handling. Public fallible APIs return structured error kinds;
avoid converting all failures into an unclassified string.

These conventions are enforced by the existing lint and binding checks.
Existing Clippy allowances in the core are documented debt, not a guarantee
that every production path is panic-free. Remove an allowance only after
addressing its uses. Safe Rust does not by itself bound memory consumption.

## Parity and evidence

The [manifest](../pillow-rs/tests/fixtures/manifest.yaml) fixes the selected
public contract. Generators produce input-only cases and coverage plans.
The runner executes a pinned Pillow oracle and the Rust-backed target in
separate processes, then compares their observations.

Inputs describe calls, arguments, assets, and selectors. Outputs, expected
hashes, timing results, and coverage receipts are execution artifacts. Never
edit expected output or omit a failing lane to make the implementation pass.

The font and codec projects maintain their own contracts. Their passing subsets
do not expand pillow-rs's promised compatibility. Read
[maturity](COMPATIBILITY.md), [coverage](COVERAGE.md), and
[benchmark methodology](BENCHMARKING.md) together when assessing a change.
