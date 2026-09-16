# Architecture and source ownership

pillow-rs has one Rust image implementation and language bindings. The
[repository map](REPO_MAP.md) locates the maintained source and generators.

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

## Image and backend boundaries

An image retains its native mode and layout. Drawing writes the native pixel
format; converting every input to RGBA changes observable behavior and cost.
Mode conversion is an explicit operation.

CPU, SIMD, and GPU execution share public semantics, with different capability
and routing limits. A backend request can include host work or fallback.
Execution receipts record requested and actual routing plus terminal completion.
A registration, shader compilation, or GPU request is not proof of native
arithmetic or source-line coverage.

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
