# GetChannel RGB serial CPU measurement (x86_64)

This is a selected diagnostic run for
`pil-image-image.getchannel.materialized.rgb-1024x768` and its exact parity case
`PIL.Image.Image.getchannel.nuanced.performance-rgb-1024x768`. It does not
represent a full benchmark or parity campaign, and it contains no code change.

## Result

| Subject | Median latency | p95 latency |
| --- | ---: | ---: |
| Pillow 12.2.0 | 0.291836 ms | 0.310921 ms |
| pillow-rs serial CPU | 0.3038615 ms | 0.311981 ms |

For this measurement, pillow-rs is about 4.12% slower than Pillow by median
(Pillow / pillow-rs = 0.9604x; difference 12.0255 µs). Treat this as a measured
gap to investigate, not an optimization result.

Parity preflight selected and executed exactly one case: 1 passed, 0 failed.
The benchmark workload also records `parity_pass` for this evidence run.

## Scope and execution

- Host: x86_64 Linux, kernel `7.0.0-38`; CPython 3.12.3; Rust 1.96.1.
- Build: release; Pillow 12.2.0; pillow-rs target revision
  `418d5f0cde51ca64a97cbd007ea731da72d68abf` (dirty worktree).
- Input: RGB, 1024×768, warm cache, single-operation chain.
- Measured boundary: `call` plus `observe-result`, including result
  materialization; 5 warmups, 20 iterations × 5 samples, concurrency 1.
- Selected backend: serial CPU only. Receipt reports `actual_backend=cpu`,
  `ExtractBand` on the `cpu` path, no fallback, and 0 vector blocks.
- Parity run: `migration-parity-benchmark-gate-24b0a5ea7ec14bc79218ae1cd53bf4ee`.
- Benchmark run: `migration-benchmark-364fcc70974d427198b3201e0b3c559d`.

## Evidence limits

The target execution receipt reports `status=partial` and
`terminal_complete=false`. Although it identifies the CPU backend and the
benchmark and parity reports completed, this receipt does not satisfy the
strict terminal proof gate. Keep this run diagnostic; do not promote it to the
generated baseline or count it as completed strict backend evidence. The gate
was not weakened.

The reported target revision is dirty. The runner's GetChannel implementation
and directly relevant call-path files match the local source at measurement
time: `pillow-rs/src/compute/pool_cpu/ops/color.rs`, `pillow-rs/src/image.rs`,
`pillow-rs/src/compute/registry.rs`, and
`pillow-rs-py/python/pillow_rs/image.py`. The local
`pillow-rs/src/compute/mod.rs` and `pillow-rs-py/src/lib.rs` also contain
unrelated telemetry/GetColors changes that differ from the runner checkout;
therefore this is not a whole-worktree identity match.

The loaded extension SHA-256 was
`db63fd7f293642e67845813a3db4e76ebaaddf559bc728d44e5907df0f474248`. The
workload record SHA-256 is
`f0203987ebe336b57fe121e3a4d8a6f2cdb1e787d8c28c2d9192cb1a8d1dd5cf`; the
canonical selected parity-case SHA-256 is
`ca5d07d413f695119196f9bdb980f2fc6bfb788173afed973c1f124907790e96`.

`benchmark.json`, raw samples, producer metadata, and attestations are retained
here. The parity report is stored as `parity.json.gz`; its uncompressed SHA-256
is `1284050825a51a680ef07bc94681bef4ef87cb7f704ca0c70c27d9b1ff4690d9`.
`SHA256SUMS` verifies the retained files. No build target or large runner
directory was copied.
