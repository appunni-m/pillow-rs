# GetChannel RGB serial CPU x86 current-source rebaseline — 2026-10-20

This is one selected, parity-gated measurement of
`pil-image-image.getchannel.materialized.rgb-1024x768` and
`PIL.Image.Image.getchannel.nuanced.performance-rgb-1024x768`. It is a current
control record, not an accepted optimization or a promoted clean baseline.

## Result

| Subject | Median | P95 | Pillow / target |
| --- | ---: | ---: | ---: |
| Pillow 12.2.0 | 0.3022405 ms | 0.338421 ms | 1.0000× |
| pillow-rs serial CPU | 0.317506 ms | 0.336681 ms | 0.9519× |

The selected call is 5.05% slower than Pillow by median. The result therefore
keeps RGB GetChannel on the x86 serial-CPU miss list. The run selected exactly
one benchmark workload and freshly passed its one parity case. The CPU
execution receipt reports `actual_backend=cpu`, one `ExtractBand` CPU
operation, complete terminal materialization, no fallback, and one 786,432
byte output allocation. The receipt also reports one mode conversion and
3,145,728 peak live host bytes. No SIMD or GPU timing was selected.

## Conditions and identity

- Host: AMD Ryzen 5 2600X, Linux x86_64, CPython 3.12.3, Pillow 12.2.0,
  Rust 1.96.1.
- Warm release run; 5 warmups, 20 iterations × 5 samples, concurrency 1.
- Timed boundary: `call;observe-result`, including GetChannel and result
  materialization/byte export.
- Benchmark run: `migration-benchmark-da99ee9d8b7d40cb83deb8ee5b441260`.
- Fresh parity run: `migration-parity-benchmark-gate-8d17578406d44364b6fd41b108594b18`
  (1 selected, 1 passed, 0 failed).
- Target revision: `418d5f0cde51ca64a97cbd007ea731da72d68abf`, dirty worktree.
  The loaded `lib_core.so` SHA-256 is
  `dee09f63e602025d70551cb8e960bb8cc0c9cc0a636e1e00b323d4de223c5d19`.
- Directly relevant local and remote source hashes match for `color.rs`,
  `image.rs`, the Python binding, and the public Python wrapper. The selected
  workload object hash (`f0203987ebe336b57fe121e3a4d8a6f2cdb1e787d8c28c2d9192cb1a8d1dd5cf`)
  and selected parity case object hash
  (`ca5d07d413f695119196f9bdb980f2fc6bfb788173afed973c1f124907790e96`) also
  match the local checkout.
- The whole fixture inputs and manifest do not match the local checkout: the
  x86 report records benchmark input SHA-256
  `fced232cf921915022a99786b4cc52a5bb4525cef6fa7cd55d82ae3ba2e12900`, parity
  input SHA-256 `ae4e8e2160df8fd0c06aa00838bfa9e1bd03dbfdc0cd4b9c694b9defb0463bda`,
  and manifest SHA-256
  `f740fbfba2f2ea927a3b14413e1fab65bc9da1daeddef061612f68fed155ee41`. The
  local hashes differ. Treat this as selected-object evidence only, not a
  whole-manifest identity match.

## Build and evidence limits

The reused checkout has no `pip` in either its virtualenv or system Python, and
no `uv`; `make build-parity` therefore could not complete through Maturin. The
existing release target was built in place with locked Cargo defaults before
the run; no checkout, target directory, wheel, or large build output was copied
to the x86 host. The benchmark/parity adapters and their schema validators did
complete successfully against that extension.

The runner's report records canonical `make migration-parity-benchmark` in
`identity.command.argv`; the actual selected invocation used
`scripts/run_migration_benchmark.py --workload-id ...` directly. The runner
currently hardcodes the canonical Make command instead of capturing the
process argv, so this field is a workflow label rather than an invocation
transcript. The selected workload, backend receipt, and parity selection are
still explicit in the reports.

Historical RGB CPU trials already cover eager dispatch, channel hoisting, safe
iterators, and spare/final-buffer writes. Eager dispatch and the safe iterator
were rejected; grouped RGB loops only reached near parity; spare-buffer trials
also regressed SIMD guards. I did not repeat those trials. The new complete
receipt confirms the gap; a different low-risk hypothesis is needed. Track the
work under the existing Kata issue `pillow-rs#jpj6`.

The compressed parity result is retained as `parity.json.gz`; its uncompressed
SHA-256 is `eaf4f09b054f9228a6a0c01eba91b4db448da6998c90ff062b3cee46a7658ba5`.
`SHA256SUMS` covers retained reports, samples, attestations, and producer
sidecars. The full parity payload compresses from 4.20 MB to 5.6 KB. No remote
files were deleted and no coverage ran. This run adds no completed optimization
to the 33-item commit/push batch.
