# GetData HSV band 2 current-source rebaseline — 2026-10-15

## Scope

This is a focused dirty-tree diagnostic for the historically slow
`pipeline-chain.terminal-read.getdata.hsv-band2-1024x768` workload. It selected
only `python-cpu` on macOS 15.7.7 ARM64, using the current local source. The
benchmark used the existing warm-cache policy: 5 warmups, 20 iterations × 5
samples, concurrency 1, and `observed_steps: band-data`. The exact strict
preflight case was
`PIL.Image.Image.getdata.performance.material-hsv-band2-1024x768` (1/1 pass).

The image source SHA-256 was
`e8670886bbf370900487407a782d6bdbd0a262dfec55dedf6289108bcd2ba009`; the
binding source hashes were `pillow-rs-py/src/lib.rs`
`068c22d3b7964dd917584118860de0034623b7f19683e36a87d997b8e8b3ca24` and
`pillow-rs-py/python/pillow_rs/image.py`
`16d7f78048f912b736d56071b5158903d962113cf2d6c1cbe305c5e69762565d`. The
release extension had already been built from this tree, and no Rust source
changed between that build and this run.

## Result

| Subject | Median | p95 | Samples |
|---|---:|---:|---:|
| Pillow 12.2.0 | 5.3952705 ms | 5.504959 ms | 100 |
| pillow-rs Python CPU | 2.4528535 ms | 2.523666 ms | 100 |

The observed ratio is 2.1996× Pillow. The receipt records one `GetData`
operation on actual CPU, terminal complete, and no fallback. `band-data` calls
the public `getdata(band=2)` and includes final result construction in the
timed step: the binding creates the Python `PyBytes` output and
`getdata_band_into` fills that final buffer directly. The Rust host-allocation
counter is zero because this final allocation is owned by Python; the receipt
reports one 786,432-byte host buffer. No intermediate Rust `Vec` is needed for
this selected-band path.

The old published 0.765807× ARM64 row used a different `image.rs` source hash
and build identity. This run shows the current dirty source clears Pillow and
the local 2× checkpoint on this host/workload, but it does not rewrite the
pinned clean matrix or rebaseline other modes, sizes, architectures, or
backends. The x86 runner was not used because its available checkout is a
separate older dirty resize-trial tree.

## Artifacts

The report, raw timing samples, exact parity report, and execution/producer
attestations are retained in this directory. Two Pillow/CPU rows were appended
to `../performance-optimization-local-runs.csv`. Kata tracking is
`pillow-rs#v2jj`; the frozen scope and result comments identify this rebaseline.
