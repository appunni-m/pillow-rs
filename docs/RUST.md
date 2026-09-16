# Rust integration

The core crate owns image operations. Bindings delegate to it, while the Rust
API uses Rust values and `Result`.

```toml
[dependencies]
pillow-rs = "=0.1.3"
```

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
retains its selected mode. The [versioned Rust reference](https://docs.rs/pillow-rs/0.1.3/pillow_rs/)
describes fallible operations, buffers, and root exports.

## Features and backends

The [Cargo manifest](../pillow-rs/Cargo.toml) defines defaults and optional
features. Codec features select image-slash-star support. `gpu` enables GPU
infrastructure; it does not make every operation native GPU work. `parallel`
enables the configured parallel paths.

The [maturity guide](COMPATIBILITY.md) distinguishes compiled features, tested
cases, and measured native dispatch. Inspect dispatch receipts for GPU timings.

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
