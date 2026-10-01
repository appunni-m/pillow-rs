# pillow-rs-parallel

This companion distribution provides the opt-in Rayon-backed Parallel CPU
extension for `pillow-rs`. Install it through the main package extra:

```sh
python -m pip install 'pillow-rs[parallel]'
```

The standard `pillow-rs` wheel remains serial CPU by default. The companion
wheel installs a separately named extension; the Python package selects it
when present. Rayon is used only for CPU execution, never for SIMD or GPU work.
