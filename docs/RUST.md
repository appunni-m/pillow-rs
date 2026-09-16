# Rust integration

The core crate owns image operations. Bindings delegate to it, while the Rust
API uses Rust values and `Result`. This guide targets **12.2.0-alpha.1**
(unreleased). The registry dependency below applies after publication; use a
[source build](INSTALLATION.md#build-the-unreleased-candidate) until then.

```toml
[dependencies]
pillow-rs = "=12.2.0-alpha.1"
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
retains its selected mode. The [published Rust reference](https://docs.rs/pillow-rs/)
describes the latest published crate. For this candidate, consult the
[source API](../pillow-rs/src/lib.rs) and migration notes below.

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

## Upgrading to 12.2.0-alpha.1

The unreleased 12.2.0-alpha.1 candidate removes deprecated Rust interfaces, so it is a
breaking upgrade from 0.1.x. It becomes available in registries after the
candidate passes release gates and its immutable tag is published.

| Removed interface | Maintained replacement |
| --- | --- |
| `Pixel::channels4` | `channels()` or `channels_mut()` |
| `Pixel::from_channels` | Native pixel constructors or `Pixel::from_slice` |
| `GenericImage::get_pixel_mut` | `get_pixel` then `put_pixel`; concrete `ImageBuffer::get_pixel_mut` remains available |
| `GenericImage::blend_pixel` | Blend native pixels directly with `Pixel::blend` and write them back |
| `Image::transform_affine` | `Image::transform_public` with method `0` and `TransformData::Affine` |
| `PipelineOp::PointOp` | `PipelineOp::Eval` |
| Deferred quantization and generator variants | `Image::quantize` and the public `linear_gradient`, `radial_gradient`, and `effect_mandelbrot` constructors |

No selected Pillow endpoint or parity requirement was removed. Benchmark
workload IDs remain stable; eager constructors and public point workflows
are counted separately from the remaining deferred Rust variants.
