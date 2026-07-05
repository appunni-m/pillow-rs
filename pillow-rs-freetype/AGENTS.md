# pillow-rs-freetype Agent Instructions

This repository is the standalone pure-Rust FreeType parity project.

## Goal

Match version-pinned C FreeType behavior with Rust runtime code.

- Pixel or bitmap byte parity where rendering produces a mask.
- Exact metric parity where metrics are exposed.
- Exact bbox/cbox parity where geometry is exposed.
- Deterministic, documented fixture generation.
- No hidden narrowing of fixture matrices or thresholds.

## Non-Negotiable Rules

- Runtime code is pure Rust.
- No `freetype-sys`, `bindgen`, `cc`, `pkg-config`, `extern "C"`, `dlopen`,
  native FreeType calls, or runtime C build hooks.
- C FreeType is an offline oracle only: fixture generation, diagnostics, and
  trace comparison.
- Never edit fixtures, expected hashes, thresholds, or tests to make code pass.
- Never add temporary debug prints to committed runtime code. Use guarded
  `log::trace!` for permanent traces.
- Do not revive legacy Pillow font backends. This project owns the FreeType
  implementation directly.

## Required Commands

Run narrow checks first, then the broader gates:

```bash
cargo test --test coverage_matrix_tests --locked -- --nocapture
cargo test --test no_runtime_ffi --locked -- --nocapture
cargo fmt -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
```

For benchmark changes:

```bash
PYTHONPYCACHEPREFIX=target/pycache python3 -m py_compile scripts/bench_freetype.py
python3 scripts/bench_freetype.py --self-test
python3 scripts/bench_freetype.py --compare-c --samples 2 --table
cargo test --test perf_benchmark_contract --locked
```

## Debugging Protocol

For parity failures:

1. Pick one font, one glyph, one size, one endpoint.
2. Dump C and Rust at the same pipeline stages.
3. Find the first divergence.
4. Read the exact C reference function.
5. Fix the Rust root cause.
6. Re-run the lane and full matrix.

Important stages:

- raw glyph load
- scaled outline before hinting
- bytecode or autohint state
- phantom points and advances
- final hinted outline
- bbox/cbox
- rasterizer cells/spans/bytes
- public bitmap metadata

## Documentation

Update docs when behavior, benchmarks, fixture generation, or harness semantics
change. Keep long playbooks in `doc/`; keep this file short and enforceable.
