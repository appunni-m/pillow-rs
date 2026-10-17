# GetColors scalar I/F ARM64 diagnostic baseline

This focused run measured only the selected `getcolors` step for scalar `I`
and `F` images at 1024×768. It used a warm public call boundary, five warmups,
20 iterations across five samples, concurrency one, Pillow 12.2.0, and the
current dirty ARM64 source. Exact parity passed 1/1 for
`PIL.Image.Image.getcolors.nuanced.analysis-scalar-if-terminal-1024x768`.

| Mode | Pillow median | Target median | Reported ratio |
| --- | ---: | ---: | ---: |
| I | 1.8908955 ms | 0.8035210 ms | 2.3533× |
| F | 2.2487295 ms | 0.7851665 ms | 2.8640× |

The timings are useful as local diagnostics, but these rows do not establish
GetColors backend execution. The untimed proof child ran all 14 parity steps;
the report attached the final `GetProjection` CPU receipt to each selected
GetColors workload. The target report's `operation_telemetry` names
`GetProjection`, despite the measurement policy selecting `i-getcolors` or
`f-getcolors`. The ledger therefore records the target rows as
`diagnostic_only_proof_step_mismatch`, with backend, operation, and resource
credit withheld. Do not use these ratios as GetColors completion or
acceleration evidence.

The current working-tree fix scopes selected-step route and operation evidence
to the policy's step IDs, preserves full-workflow terminal completion as a
separate check, and keeps fallback aggregation across every proof receipt.
Three focused contracts passed in
`scripts/test_migration_benchmark_selection.py::BenchmarkExecutionProofScopeTests`.
No image benchmark or parity rerun was needed for this proof-tool correction.

## Identities and reports

- Benchmark report: `control.json`, run
  `migration-benchmark-202902d974e849a592b21dca147714fe`.
- Canonical report SHA-256:
  `534aab4ab92b156002df82c7ba240afe8b1d5e114fa49f12309175d5695bdf5d`.
- Parity report: `control.parity.json`, run
  `migration-parity-benchmark-gate-e3d54af884104bffb5eb8192f8500fb1`.
- Raw samples: `control.raw-samples.json`.
- Execution and producer identities: `control.attestation.json` and
  `control.producer-attestation.json`.
- Source revision: `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`, dirty.
- Loaded `pillow_rs._core` SHA-256:
  `e7a1ce76dd97149c6a86b0b69db926fee58b2b1a93740cbd60fe0ee6d6f6839e`.
- Workload manifest SHA-256:
  `ff0d8fa434e60ed844d9d3606396919e28e8b59bdcc590c80d0d73564ec5d885`.
- Benchmark inputs: `pipeline-operations.json` SHA-256
  `7eaf80ff5b3b32123c066d7e52f8ce7dcbbd359ff716c116df0256c694bae838`;
  parity input SHA-256
  `ae2b332e03ddb8b5531c9123ac5b69970258828c65323269830a32666189c1c8`.
- Focused case:
  `PIL.Image.Image.getcolors.nuanced.analysis-scalar-if-terminal-1024x768`.
- Workloads: `pipeline-chain.terminal-read.getcolors.scalar-i-1024x768` and
  `pipeline-chain.terminal-read.getcolors.scalar-f-1024x768`.

The local-run ledger contains 7,792 data rows after these diagnostic entries.
Kata tracks the attribution defect and fix in `pillow-rs#e1p3` and
`pillow-rs#y3tm`.
