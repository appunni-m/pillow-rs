# pillow-rs Documentation Plan

This plan defines the documentation standard for `pillow-rs`. Passing
`missing_docs` is the floor. The real goal is documentation that prevents wrong
changes: callers should understand input modes, output layout, failure modes,
Pillow compatibility, and the boundary between core Rust code and binding code.

The standard follows Rust API Guidelines and rustdoc conventions:

- Function docs include `# Errors`, `# Panics`, and `# Safety` when relevant.
- Examples should compile as doctests unless explicitly marked `no_run`.
- Intra-doc links should connect related types instead of leaving plain text
  names.
- Public-but-internal APIs must say why they are public and what invariants
  callers must preserve.

## Documentation Contract

Every public item must be documented at the right level. Do not write the same
style of docs for a public user API, a shader dispatch key, and a parity helper.

### Surface API

Use this level for modules, types, and functions a binding crate or downstream
Rust caller is expected to use.

Surface docs must answer:

- What problem does this item solve?
- What inputs are accepted, including units, modes, layout, and ownership?
- What output is returned, including pixel format, byte layout, and dimensions?
- Which Pillow behavior is being mirrored?
- What errors can be returned?
- What panics are possible, if any?
- Which related item should the caller use next?

Examples in this crate:

- `Image::new`, `Image::open`, `Image::open_bytes`, `Image::convert`,
  `Image::save`, `Image::tobytes`.
- `Font` and `font::imagingft` APIs used by Python and JS bindings.
- `CheckedDims`, because callers must use it before allocating image buffers.

### Internal Public API

Use this level for items that are public only because tests, generated code, or
binding crates need access.

Internal public docs must answer:

- Why is this public?
- Which caller is expected to use it?
- What invariants must the caller preserve?
- Is this stable API or implementation detail?

Examples in this crate:

- `pipeline::PipelineOp`: internal-public operation descriptor used by lazy
  image pipelines and compute backends.
- `compute::registry::OpId`: GPU shader dispatch identifier.
- `compute::registry::OpEntry`: backend implementation wiring.

Internal-public docs must include the words `Internal contract` in the module
or type docs. This makes it clear to humans and AI agents that the API is public
for crate wiring, not because it is a polished downstream interface.

### Parity-Sensitive API

Use this level when behavior must match Pillow, PIL `_imagingft.c`, or another
oracle exactly.

Parity-sensitive docs must answer:

- Which upstream behavior is mirrored?
- Which input and output units matter for parity?
- Which byte layout is expected?
- Which tests or fixtures protect the behavior?
- Which behavior is intentionally incomplete and visible as a baseline?

Examples in this crate:

- Color conversion formulas in `color`.
- `font::imagingft` connector behavior.
- Drawing into native image modes.
- `Image.convert`, `Image.getbbox`-style font surfaces, and palette handling.

### Generated or Mechanical API

Use this level for operation registries, repeated compute operations, and
macro-defined public items.

Generated docs should still be meaningful. The operation definition should carry
the source-of-truth fields that a macro can turn into rustdoc:

- summary
- input modes and argument meaning
- output mode or layout
- Pillow equivalent
- failure modes

Do not generate docs that only restate function names.

Current rule: do not use `#[allow(missing_docs)]` in `pillow-rs`. If an item is
too mechanical to document by hand, the operation metadata or macro should emit
its docs.

## Standard Rustdoc Shape

Use conventional rustdoc sections where they add information:

```rust
/// One-sentence summary of the operation and its Pillow relationship.
///
/// # Inputs
///
/// - `arg`: Meaning, units, accepted range, and ownership.
///
/// # Returns
///
/// Returned value, pixel layout, dimensions, and mode semantics.
///
/// # Errors
///
/// Conditions that produce [`PilError`] or another error type.
///
/// # Panics
///
/// Invariants that are asserted instead of returned.
///
/// # Examples
///
/// ```
/// # use pillow_rs::...;
/// ```
```

Prefer intra-doc links such as [`crate::Image`], [`crate::PilError`], and
[`crate::CheckedDims`] over plain text names.

Avoid non-standard headings unless they are clearer than a long paragraph. The
preferred crate-specific headings are:

- `# Pillow Compatibility`
- `# Internal Contract`
- `# Mode And Layout`
- `# Errors`
- `# Panics`
- `# Examples`

## Module Documentation Template

Each public module should start with `//!` or have a doc comment at the `pub mod`
declaration that explains:

- domain and Pillow equivalent
- most important public entry points
- input/output conventions
- implementation boundary

Example:

```rust
//! Pillow-compatible color parsing and color-space conversion.
//!
//! This module accepts Pillow mode strings such as `"RGB"` and `"L"` and
//! returns Rust primitives or `image-slash-star` buffers. It does not inspect
//! Python objects or file paths; binding crates perform that conversion before
//! calling into core.
```

## Good And Bad Examples

Bad:

```rust
/// GPU invert operation.
Invert,
```

This only restates the name. Better:

```rust
/// Dispatch key for the WGSL shader that inverts color channels in-place.
///
/// This variant must stay aligned with `PipelineOp::Invert`, `variant_key`,
/// and the embedded shader registry.
Invert,
```

Bad:

```rust
/// Output width in pixels.
w: u32,
```

This is acceptable only when rustdoc requires a field doc and the surrounding
variant/module docs already explain the operation. Better when the field has
real semantics:

```rust
/// Right edge of the crop box, exclusive, in source image pixels.
right: u32,
```

Bad:

```rust
/// Converts image.
pub fn convert(...)
```

Better:

```rust
/// Converts this image to a Pillow mode such as `"L"`, `"RGB"`, or `"1"`.
///
/// # Pillow Compatibility
///
/// Palette images with no explicit destination mode follow Pillow's default
/// `P -> RGB` behavior.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when the destination mode, matrix length,
/// or dither option is invalid.
```

## Rollout Order

1. Crate and module docs for `pillow-rs/src/lib.rs`.
2. Core public types used by bindings: `Image`, `Font`, `PilError`,
   `CheckedDims`, `PixelFormat`.
3. User-facing operation modules: `color`, `draw`, `ops`, `formats`.
4. Mechanical compute registry docs through metadata-backed macro generation.
5. Doctest examples for stable APIs.
6. Promote documentation checks from warning-only to a stricter CI gate once
   the existing public surface has useful coverage.

## Execution Slices

Use these slices when improving docs:

1. **Mental model first**: crate docs, module docs, and diagrams/tables in prose.
2. **Public workflows**: examples for image creation, conversion, drawing, text,
   encoding, and raw bytes.
3. **Contracts**: `# Errors`, `# Panics`, mode/layout details, and coordinate
   semantics.
4. **Internal-public infrastructure**: pipeline, compute registry, dispatch IDs,
   and macro-generated operation definitions.
5. **Parity notes**: exact formulas, fixture names, and upstream behavior.
6. **Generated docs**: move repeated operation docs into metadata or macros.

## Verification

Run these after each documentation slice:

```bash
make migration-parity-test-all-backends
cargo doc -p pillow-rs --no-deps
```

The strict gate is mandatory for documentation work:

```bash
RUSTDOCFLAGS="-D warnings" cargo doc -p pillow-rs --no-deps
make migration-parity-test-all-backends
```

There must be no `#[allow(missing_docs)]` in `pillow-rs`.

```bash
rg "allow\\s*\\(\\s*missing_docs\\s*\\)|allow\\s*\\[\\s*missing_docs\\s*\\]" pillow-rs/src
```

## Anti-Patterns

Do not add:

- docs that repeat the name: "Gets the width."
- examples that do not compile unless explicitly marked `ignore`
- hidden behavioral promises that are not covered by tests or fixtures
- vague compatibility claims such as "Pillow-like" without naming the surface
- duplicate paragraphs across many functions when a module-level contract would
  explain the shared behavior better

Useful docs should reduce mistakes. They should tell a caller what to pass, what
they receive, why the item exists, and where parity constraints come from.

## Review Checklist

Before marking a documentation change complete:

- Public `Result` methods have `# Errors`.
- Public panic paths have `# Panics`.
- Examples compile or are explicitly marked `no_run`.
- Intra-doc links resolve under `RUSTDOCFLAGS="-D warnings"`.
- Public-but-internal APIs say `Internal contract`.
- Pillow compatibility claims name the exact Pillow behavior or surface.
- Mode strings, byte layout, and coordinate inclusivity are documented where
  they affect behavior.
- No docs merely repeat the identifier.
- No `#[allow(missing_docs)]` exists in `pillow-rs/src`.
- The imagingft fixture test still passes when font/text behavior was touched.
