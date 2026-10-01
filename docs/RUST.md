# Rust integration

<!-- release:summary -->
**Latest release: [12.2.0-alpha.5](https://github.com/appunni-m/pillow-rs/releases/tag/v12.2.0-alpha.5).**
<!-- /release:summary -->

Install the core image-processing crate from crates.io. Requires Rust 1.96.1
or newer. Public methods use Rust values and `Result`.

<!-- release:cargo -->
```toml
[dependencies]
pillow-rs = "=12.2.0-alpha.5"
```
<!-- /release:cargo -->

## First operation

```rust
use pillow_rs::Image;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let image = Image::new(4, 4, "RGB", (0, 128, 255, 255))?;
    assert_eq!(image.size()?, (4, 4));
    let gray = image.convert("L", None, None, None, None)?;
    assert_eq!(gray.tobytes()?.len(), 16);
    Ok(())
}
```

The constructor takes width, height, mode, and an RGBA color tuple. The image
retains its selected mode.

<!-- release:rust-api -->
[Rust API reference](https://docs.rs/pillow-rs/12.2.0-alpha.5/pillow_rs/).
<!-- /release:rust-api -->

The reference describes arguments, return values, and errors.

## Features and backends

The [Cargo manifest](../pillow-rs/Cargo.toml) defines defaults and optional
features. Codec features select image-slash-star support. `gpu` enables GPU
infrastructure; it does not make every operation native GPU work. The default
build keeps Rayon `parallel` disabled; enable it explicitly with
`--features parallel` only in runs reported as Parallel CPU. Keep those results
separate from default single-thread SIMD and GPU measurements. The SIMD
backend keeps row scheduling serial even when `parallel` is enabled, and GPU
readback never schedules Rayon work.

The [compatibility guide](COMPATIBILITY.md) explains the supported scope.
GPU availability and acceleration depend on the operation and host.

## Ownership and I/O

Core APIs accept image values, byte buffers, mode strings, and font bytes.
Applications own filesystem and network I/O. Use the root crate's public
re-exports instead of private implementation module paths.

Mode changes and byte conversion can allocate. A method name does not establish
zero-copy behavior or recoverable out-of-memory handling. Follow the exact
method contract and apply limits appropriate to your inputs.

## Integration checks

Run your corpus with the features and target you will ship. The published
Python and WASM evidence does not establish every Rust feature/target
combination. Report regressions with the smallest input and public call sequence.

## New integrations

Treat each alpha as a greenfield integration. Pin an exact version, use the
current API reference, and validate the modes, arguments, outputs, and errors
your application needs before adopting another alpha. No alpha-to-alpha
migration path or compatibility guarantee is provided.
