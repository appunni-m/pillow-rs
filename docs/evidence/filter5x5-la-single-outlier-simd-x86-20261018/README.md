# LA Filter5x5 single-outlier SIMD x86 recheck — 2026-10-18

This records a focused native-x86 check of the retained SIMD specialization
for native LA with kernel [1.0; 24] + [2.0], scale 25, offset 0. It is one
current-source measurement, not an x86 A–B–B–A; it verifies the selected
workload against regular Pillow but does not establish a causal x86 speedup.

## Frozen scope

- Workload: pipeline-op.filter.material-la-nonmatching-5x5-1024x768.
- Primary parity case: PIL.Image.Image.filter.nuanced.la-mode-kernel-5x5-nonmatching-noise-1024x768.
- Odd-width guard: PIL.Image.Image.filter.nuanced.la-mode-kernel-5x5-nonmatching-odd-width-7x5.
- Measurement: public call plus terminal materialization; warm release, 5
  warmups, 20 iterations × 5 samples, concurrency 1.
- Backend profile: python-simd only.

## Result

| Subject | Median | P95 | Throughput | Pillow / subject |
| --- | ---: | ---: | ---: | ---: |
| Pillow 12.2.0 | 24.723155 ms | 30.770257 ms | 40.447916 ops/s | 1.000× |
| pillow-rs SIMD | 2.660280 ms | 2.903564 ms | 375.900287 ops/s | 9.293441× |

The benchmark gate passed exact primary parity. The separate 7×5 odd-width
guard passed 1/1. The target receipt reports requested and actual SIMD,
terminal completion, no fallback, one Filter5x5 operation, 97,028 vector
blocks, and 3,056 scalar-tail pixels. It reports two host buffers totaling
3,145,728 bytes. Allocation-event counters are zero on this route and do not
prove that no Rust allocation occurred.

This current-source row no longer reproduces the historical x86 SIMD-vs-Pillow
miss for this exact LA workload. Pillow-SIMD has no valid comparison here:
the recorded compatibility gate for this workload fails. Other modes,
workloads, backends, and operation-wide completion remain open.

## Run and source identity

- Host: AMD Ryzen 5 2600X, Linux x86_64, CPython 3.12.3.
- Checkout: revision 418d5f0cde51ca64a97cbd007ea731da72d68abf, dirty.
- Build: make build-parity on the x86 runner; release wheel
  pillow_rs-12.2.2-cp38-abi3-linux_x86_64.whl.
- Loaded extension SHA-256:
  a856b319682bd41ed7ecded3bb0c9c62e39fd162f5add7ae42b825389d11afb7.
- native_filter.rs SHA-256:
  57bc1de950ebc01284baae235a5b8431e65c12ed48d12383cf89a44839192977.
- SIMD adapters.rs SHA-256:
  6391c163ba481eba90bff7d2f889dda421b63af39235934bac1b518ab304420d.
- pool_cpu filter.rs SHA-256:
  ef3930dce6bebb196e47b4cc30c702e283e2f962552acba36295dabb63f30470.
- Benchmark fixture SHA-256:
  9f0a8fdbef7c4eb921ac9897a7ddd1810c213df4fe962fb8e7d9870f541202a8.
- Parity input SHA-256:
  98b31711fdc2b9997af256c49f334decb9665abcfcc24c52251f1cd2918b148a.

## Artifacts

- Benchmark run: migration-benchmark-e508bd4b69e645a1bd07ab926ee656d8.
- Benchmark report SHA-256: 966d9feaf9f49b459fd7a4a26bd22cd3898e50cdc6f7b9608bbf11e5999140c6.
- Raw samples SHA-256: b1fc85a6cf14ffb0af729179f8a34062708e2cbe3f7c3ddf23cdfee4d14ecff3.
- Primary parity report SHA-256:
  b685e5d22ad80c9c43459b34f8532f6d635160588ceb383aa1a74198b013972c.
- Odd-width parity run: migration-parity-0e1c42c496244f4abcafcb897b4fc51c.
- Odd-width parity report SHA-256:
  0f4adeabaeccd1d68362fc81f90d317b530d741b546195e6e793e7e4a6add683.

The benchmark, raw samples, run attestations, and compact odd-width parity
report are retained in this directory. The full primary parity JSON is 8.4 MB
because it embeds image bytes; it remains on the native runner at
/home/appunni/work/pillow-rs-ycbcr-20261018/build/migration-parity/la-nonmatching-5x5-simd-x86-current-20261018/
and was not copied. No build target or binary was transferred.
SHA256SUMS.txt validates every captured JSON artifact in this directory.
