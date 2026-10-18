# Native-LA Filter5x5 x86 CPU verification — 2026-10-17

This adds one native-x86 measurement for the retained single-outlier kernel
specialization. It confirms this workload's current CPU latency and parity;
it is not an A-B-B-A comparison or an operation-completion claim.

## Frozen workload and identity

- Workload: `pipeline-op.filter.material-la-nonmatching-5x5-1024x768`.
- Full-size parity case:
  `PIL.Image.Image.filter.nuanced.la-mode-kernel-5x5-nonmatching-noise-1024x768`.
- Odd-width parity case:
  `PIL.Image.Image.filter.nuanced.la-mode-kernel-5x5-nonmatching-odd-width-7x5`.
- Mode and size: LA, 1024×768 benchmark; LA, 7×5 edge guard.
- Kernel: `[1.0; 24] + [2.0]`, scale 25, offset 0.
- Measurement boundary: public `call;materialize`, including result materialization.
- Sampling: warm cache, 5 warmups, 20 iterations × 5 samples, concurrency one
  (100 observations per subject).
- Oracle: Pillow 12.2.0, CPython 3.12. Target: serial CPU, default Cargo
  features; Parallel CPU is disabled.
- Host: AMD Ryzen 5 2600X, Linux x86_64, CPython 3.12.11.
- Source revision: `1c9203b11e789059b139bbd4772bc3184cf37511` (dirty because
  focused benchmark/parity fixture changes were present; `filter.rs` itself
  matches the committed source hash below).
- `filter.rs` SHA-256:
  `aeacce762f0bcaa30c853e7fb70d1d649b30ee28f9bb2a8a30b3fd43db846175`.
- Loaded `_core.abi3.so` SHA-256:
  `5b117f519887f77ec3af54a57f7858e57d522be0edc964f041e5aa41c31bf502`.
- Benchmark run: `migration-benchmark-0c8b9332d70742dab590a945f91b826e`.
- Full-size parity gate:
  `migration-parity-benchmark-gate-abceb9e7d9db4efd9419413d1a230378`.
- Odd-width strict CPU parity run:
  `migration-parity-d35d0c85f8ce44bfae4836d6df40a34c` (1/1 passed).

## Result

| Subject | Median | P95 | Throughput | Pillow / subject |
| --- | ---: | ---: | ---: | ---: |
| Pillow | 23.145001 ms | 25.262837 ms | 43.205875 ops/s | 1.000× |
| Serial CPU | 6.452647 ms | 6.527678 ms | 154.975160 ops/s | 3.587× |

The target receipt proves actual CPU execution for one Filter5x5 operation,
terminal completion, and no fallback. The selected full-size parity gate
passed, and the added odd-width 7×5 guard passed exactly against Pillow.

The resource receipt reports two host buffers totaling 3,145,728 bytes and a
3,145,728-byte peak. Its host-allocation counters are zero; those counters do
not account for every Rust scratch allocation and must not be read as proof
that the call allocates nothing. The timed boundary still includes the public
call and output materialization.

## Limits and next proof

This was a single candidate run, not an x86 A-B-B-A trial, so it does not
establish a causal x86 speedup over an x86 baseline. Raw timing samples were
not retained (`raw_samples_ref` is null); the report's 100-sample aggregate
statistics are preserved. Do not rerun only to collect raw samples.

This result covers one LA workload and strict CPU execution only. SIMD, GPU
latency and sustained throughput, Parallel CPU, other modes, sizes, and edge
contracts remain open. Keep this result in the local-run ledger; it does not
replace or modify the clean published operation matrix.

See `SHA256SUMS.txt` for local artifact hashes and
`../../PERFORMANCE_CAMPAIGN.md` for the campaign-level checkpoint.
