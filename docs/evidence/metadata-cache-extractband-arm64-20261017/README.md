# GetChannel metadata-cache pipeline current-source diagnostic — ARM64, 2026-10-17

## Disposition

The historical 2.62×-slower-than-Pillow row is not reproduced by this current
source median. An exact parity case now covers the composed workflow, and a
CPU-only current-source run measured 73.562 µs median versus 93.750 µs for
Pillow (1.274× Pillow/CPU). This is one six-sample diagnostic on a dirty ARM64
checkout, not operation completion or stable latency evidence. CPU p95 was
126.583 µs versus Pillow's 117.500 µs, so the tail remains unresolved. No
runtime optimization was attempted.

## Frozen workflow and parity

- Workload: `pipeline-chain.metadata-cache.extractband-rgba`
- Exact parity case: `PIL.Image.Image.getchannel.pipeline.metadata-cache-extractband-rgba-256x256`
- Profile: serial `python-cpu`
- Input: create constant RGBA 256×256 `(17,83,149,211)`, extract channel 1,
  read `mode` and `size` three times each, then call `tobytes()`.
- Timing boundary: `whole_workflow`, including image creation, extraction,
  property calls, allocations, and byte materialization.

The parity case uses the same nine workflow steps as the benchmark input. It
observes the extracted image in addition to the six metadata values and final
bytes, so the exact GetChannel output, mode `L`, and size `(256,256)` are
compared with Pillow. The focused comparison passed 1/1 on CPU. The benchmark
input itself still declares `successful_execution`; its own execution gate is
not an exact-parity proof. The separate passing case is the parity evidence.

The prior clean snapshot (revision `310a6d643bdd4d06a7748cbd81466ab26caf4ede`)
reported Pillow 114.376 µs and CPU 299.396 µs, but its correctness gate was
`successful_execution`, not pixel parity. The current diagnostic uses source
revision `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31` with a dirty tree, so it is
not a replacement for the clean published matrix and cannot establish which
change caused the difference.

## Current-source measurement

The workload retained its original warm-cache contract: one warmup, three
iterations × two samples (six timed samples), concurrency one. It is below the
sample count used for candidate A/B/B/A decisions; do not infer stable tail
behavior from these six points.

| Subject | Median | P95 | Receipt |
| --- | ---: | ---: | --- |
| Pillow 12.2.0 | 93.750 µs | 117.500 µs | oracle |
| pillow-rs CPU | 73.562 µs | 126.583 µs | actual CPU, terminal complete, no fallback |

The CPU receipt contains one completed `ExtractBand` operation, one mode
conversion, one host allocation of 65,536 bytes, and two host buffers totaling
327,680 bytes. Upload and readback are both zero. The timed terminal is
`tobytes()`, so output materialization is included.

## Identity and artifacts

Host: `Lazys-MacBook-Pro.local`, macOS 15.7.7 ARM64, CPython 3.12.13. The
target extension loaded by both the parity and benchmark producers had SHA-256
`4e594a52264cda6166da3adf2dcb2241dcd9aec4e4ca3102d0e01a45d4204cf9`. Pillow
was 12.2.0. Parity input SHA-256 is
`c9cd37e39695fa309230dcbd497e3a7a47d2706378d756ec08fb3aad81e67d6d`; benchmark
input SHA-256 is
`5518e26b243dc3708095e09b8998cc6f5976f6c6cbc1b047529c5dc3d2e46350`.

- Parity run: `migration-parity-dd9120923fa14666b054433a2fc8dc0b`
- CPU benchmark run: `migration-benchmark-e04f705bdf374ae7ad18012ccee95f4d`
- Passing parity report SHA-256: `4d5827f0de22222f6c8beb9a02f4012fb214955ec91f0ea1b1eb23940d601158`
- Parity attestation SHA-256: `6d048814794c05661e2dd75d3f26c0641196ecd1821ce19ac2a8f3299b2ac48f`
- Benchmark report SHA-256: `01dbdbaa09b418ca61a18aec3dfe1542cff5f329f51cd2cd535aa1266990c169`
- Benchmark raw samples SHA-256: `16ee75fd47fcbe16b19bdb68c1a2197d446f14e9dec354d547ec42e66668a131`

The initial parity preflight correctly rejected a fixture that did not observe
its primary GetChannel result; the case was corrected without weakening it.
That failed invocation had already created a producer bundle, so reusing its
output path failed with `FileExistsError`; the passing run uses distinct
`parity-pass.*` artifacts. This is a proof-runner artifact-ordering defect to
track separately. `cpu-rebaseline-execution-gate.json` is empty because the
benchmark input is a workflow with a successful-execution gate; it is not a
parity result. Only `parity-pass.json` is the parity proof.

Two subject rows are in the local-run ledger. The published operation matrix
was not regenerated from this single dirty-source workload. Operation credit
remains 0 and commit/push batch remains 0/33.
