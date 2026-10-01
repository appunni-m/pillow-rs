# pillow-rs-parallel

This companion distribution provides the opt-in Rayon-backed Parallel CPU
extension for `pillow-rs`. It is installed through the main package's
`parallel` extra after matching standard and companion wheels are published.
For pre-release versions, enable pre-release resolution:

```sh
python -m pip install --pre 'pillow-rs[parallel]'
```

The standard `pillow-rs` wheel remains serial CPU by default. The companion
wheel installs a separately named extension; the Python package selects it
when present. Rayon is used only for CPU execution, never for SIMD or GPU work.
