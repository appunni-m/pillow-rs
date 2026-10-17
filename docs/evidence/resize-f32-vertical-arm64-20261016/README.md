# F32 Resize vertical-pass current-source ARM64 rebaseline

This is a focused control-source rebaseline for the full public resize call. It
does not measure an optimization candidate or close Resize.

## Frozen scope

- Workload: `pipeline-chain.resize-native-f32.bicubic-noise-1024x768`
- Boundary: `observed_steps`, `call;materialize`
- Input: mode `F`, 1024×768, two-operation pipeline, warm cache
- Backend: Serial CPU; release build; concurrency 1
- Sampling: five warmups, 20 iterations, five samples (100 latency samples)
- Pillow: 12.2.0
- Selected parity case: `PIL.Image.Image.resize.nuanced.f-resize-bicubic-noise-1024x768`
- Parity: 1 selected, 1 executed, 1 passed

## Result

| Subject | Median | P95 | Pillow speedup |
| --- | ---: | ---: | ---: |
| Pillow | 3.044 ms | 3.509 ms | 1.000× |
| Serial CPU | 0.929 ms | 1.010 ms | 3.277× |

The execution receipt reports actual backend `cpu`, one completed `Resize`,
terminal materialization, and no fallback. It records two resize coefficient
cache misses, two host buffers totaling 3,932,160 bytes, no upload or readback,
and no fused operation. The timing covers the full selected public pipeline
through materialization.

## Identity and limitations

- Dirty source revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`
- `pillow-rs/src/compute/pool_cpu/ops/geometry.rs` SHA-256:
  `f1089848ee2edea2ca4cd9f8a1fcdbe5c859e343730f6783f1f0e22c05e45486`
- Benchmark run: `migration-benchmark-856258be7922415cb1b7ef7fc1b4619d`
- Parity gate: `migration-parity-benchmark-gate-6996b566c07d4c37933e5b9c9ddcc5db`

This ARM64 baseline replaces neither the old x86 evidence nor its missing
source identity. The earlier x86 report was produced from a different dirty
`geometry.rs` source and must not be used to attribute current x86 performance.
The synthetic fixed-eight-tap vertical kernel probe measured about 1.45× on
ARM64, but it is kernel-only evidence. The next candidate evaluation needs a
same-source native-x86 run if an appropriately identified runner is available.
The separate parity case
`PIL.Image.Image.resize.nuanced.modes-F-normal-4-gap-two` was not selected in
this run.

## Artifacts

- `A1-control.json`: benchmark report
- `A1-parity.json`: selected-case parity summary
- `A1-raw-samples.json`: raw measurements
- `A1.attestation.json`: benchmark invocation and runtime attestation
- `A1.producer-attestation.json`: producer identity
- `A1.producer-attestation.json.bundle.json`: producer bundle manifest

No repository tests, coverage, or full benchmark/parity campaigns ran for this
rebaseline. Two subject rows were added to
`docs/evidence/performance-optimization-local-runs.csv` as diagnostic current
source evidence; no operation completion credit was recorded.
