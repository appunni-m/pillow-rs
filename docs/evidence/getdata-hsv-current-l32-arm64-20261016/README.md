# GetData HSV current-source identity refresh — 2026-10-16

This run refreshes the exact 1024×768 HSV band-2 row after the L/32 borrowed
mode guard changed `image.rs`. It is a dirty-tree current-source measurement,
not a clean release-matrix row or an A–B–B–A optimization claim.

## Frozen contract and result

- Workload: `pipeline-chain.terminal-read.getdata.hsv-band2-1024x768`
- Parity case: `PIL.Image.Image.getdata.performance.material-hsv-band2-1024x768`
- Profile: serial `python-cpu`; requested and actual backend were CPU.
- Boundary: warm `band-data`; this includes final Python bytes allocation and
  fill. Image setup is outside the timed step.
- Policy: five warmups, 20 iterations × five samples, concurrency one; 100
  observations per subject.
- Pillow 12.2.0 median: 5.359291 ms.
- pillow-rs CPU median: 2.4936455 ms (2.149179× Pillow).
- Exact parity passed 1/1. The receipt records one completed `GetData`, terminal
  completion, no fallback, and a 786,432-byte output buffer. The target uses no
  mode conversion. Instrumented host allocations are zero; this does not mean
  that constructing the final Python bytes object is allocation-free.

## Identity and limits

The target was built from revision `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`
with a dirty worktree. `pillow-rs/src/image.rs` SHA-256 is
`9bde8c50db45d4239a57e7ca3a52f37c279a9200f2d036f5875f66392c78e826`; the
loaded `pillow_rs._core` SHA-256 is
`b1619f942d490e573c532a5da8da0b2a709ceb3248d676272f110266b25c5fd1`. The
manifest, benchmark input, and parity input SHA-256 values are respectively
`ff0d8fa434e60ed844d9d3606396919e28e8b59bdcc590c80d0d73564ec5d885`,
`7eaf80ff5b3b32123c066d7e52f8ce7dcbbd359ff716c116df0256c694bae838`, and
`ae2b332e03ddb8b5531c9123ac5b69970258828c65323269830a32666189c1c8`. The
oracle is Pillow 12.2.0 on CPython 3.12; `PIL._imaging` SHA-256 is
`ed534830281dd092be3755632eac8422cdee53d2d8f045ff1779a7d795414d26`.

The worktree's `docs/evidence/benchmark.json` and generated operation matrix
must not be treated as the pinned clean source for the previously reported
0.765807× HSV row. The file in Git at revision
`310a6d643bdd4d06a7748cbd81466ab26caf4ede` has raw SHA-256
`c1c4f2c3126e4c25f5282d21e1541a4c041dbd5996f5018e2dd771fc9ee8c124`, report
revision `10352de697c03fdbbda515fc5ae6cd1e440aeed8`, and measured source SHA
`607e783f4ef67ca53223374b4ceba5c13eff843efff8e8396f7e7cfa2f70e9c8`. Its
HSV medians are 7.635917 ms for Pillow and 4.0315415 ms for CPU, or 1.894×.
The worktree copy has a different raw hash and different row. Keep those
snapshots separate; this local result does not rewrite either matrix.

The benchmark, raw samples, compressed parity result, execution attestations,
and producer records are retained beside this README. Run and evidence
identities are in the benchmark JSON and sidecars. Kata tracking is
`pillow-rs#a53w`. This adds no completed operation to the 33-item Git batch.
