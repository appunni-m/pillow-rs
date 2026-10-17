# F32 Resize serial vertical traversal screen — rejected

This is a focused ARM64 public-call comparison for the serial CPU-only change in
`f_resize_vertical_pass`. The candidate replaced the tap-major row sweep with
per-output-pixel calls to the existing vertical sampler. The parallel branch,
SIMD adapters, GPU, horizontal pass, coefficients, tap order, and materialized
F32 boundary were not changed by this candidate.

## Frozen scope and execution

- Workload: `pipeline-chain.resize-native-f32.bicubic-noise-1024x768`.
- Boundary: public `call;materialize`; mode F, 1024×768 to 512×384, warm release.
- Backend: serial CPU, concurrency 1; five warmups and 20 iterations × five samples.
- Main exact parity case: `PIL.Image.Image.resize.nuanced.f-resize-bicubic-noise-1024x768`.
- Candidate edge guard: `PIL.Image.Image.resize.nuanced.modes-F-normal-4-gap-two`.
- Candidate parity passed 2/2; current control parity passed 1/1.
- Every target benchmark receipt says actual backend `cpu`, terminal materialization complete, and zero fallback reasons.

## Measurements

| Run | Source state | Pillow median | Serial CPU median | CPU / Pillow | CPU throughput |
| --- | --- | ---: | ---: | ---: | ---: |
| B1 | Candidate | 2.900062 ms | 1.233146 ms | 2.353× | 810.93 ops/s |
| B2 | Candidate | 2.794313 ms | 1.252938 ms | 2.229× | 798.12 ops/s |
| A2 | Reverted current-source control | 2.772813 ms | 0.898938 ms | 3.085× | 1112.42 ops/s |

Against this current-source control, candidate CPU latency is 37.2% slower in
B1 and 39.4% slower in B2. The candidate is rejected. The benchmarked control
and candidate differed only in this serial vertical-pass block. The exact x86
trial was not run after this repeatable full-call ARM64 regression; Resize's
native-x86 published row remains an independent unresolved measurement.

The prior ARM64 control artifact in
`../resize-f32-vertical-arm64-20261016/` is retained separately. It was not
used to calculate these percentages. This screen is diagnostic and grants no
operation-completion credit.

## Source and build identities

- Dirty revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`.
- Candidate `geometry.rs` SHA-256: `b2b81a2d263c90e6689a1d093f07b481a263d4ad181a90d44e62c9e9af88051e`.
- Current control `geometry.rs` SHA-256: `05d2bffe112a0b1870d667045d35a87cc5f2ace4135f72e3f85f1dbf58203060`.
- Candidate `_core.abi3.so` SHA-256: `c200c3cb0264f25bdcc11fcb157530d248c072ab244d7a5fe7bfa7f65d45078f`.
- Control `_core.abi3.so` SHA-256: `49b07eee7672319eb7455fb1544a8fab5d8c3ac06231f2be358455bc961ad94a`.
- Pillow oracle: 12.2.0; host: macOS arm64.

The three benchmark reports, raw samples, execution attestations, producer
attestations, and compressed parity reports are stored beside this README.
