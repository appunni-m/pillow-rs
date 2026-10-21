# RGB Thumbnail PutPixel-prefix fusion: native x86 A–B–B–A

This is a focused causal check for the existing RGB `PutPixel`-prefix / `Thumbnail` SIMD fusion candidate. It uses one public whole-workflow workload and does not establish completion of `PIL.Image.Image.thumbnail`.

## Frozen scope

- Host: AMD Ryzen 5 2600X, Ubuntu x86_64; CPython 3.12.3, Pillow 12.2.0, Rust 1.96.1.
- Source base: dirty checkout at `418d5f0cde51ca64a97cbd007ea731da72d68abf` on the native runner.
- Workload: `pipeline-op.thumbnail.material-rgb-1024x768`.
- Parity case: `PIL.Image.Image.thumbnail.nuanced.performance-large-rgb-1024x768`.
- Policy: warm whole-workflow boundary; 5 warmups, 20 iterations × 5 samples, concurrency 1.
- A controls disable only SIMD fusion admission in `pillow-rs/src/compute/mod.rs`; B restores it. Control and candidate `compute/mod.rs` SHA-256 values are `cacedad6f77589c8c42865905a4ba411d99a0fa7367738db7217a994842dab49` and `785d13bfed145a79d7584d78a19d9e23f7ed9f8f849c5e8296a2e744e737ec07`. The experiment manifest records the same non-candidate-source hash for all four legs: `5ce5b9076829ff0747c65964d3fe3d4e335d12425bfd5e3837716228830f3bf2`.
- Workload manifest SHA-256: `ff0d8fa434e60ed844d9d3606396919e28e8b59bdcc590c80d0d73564ec5d885`; benchmark input SHA-256: `9f0a8fdbef7c4eb921ac9897a7ddd1810c213df4fe962fb8e7d9870f541202a8`; parity input SHA-256: `98b31711fdc2b9997af256c49f334decb9665abcfcc24c52251f1cd2918b148a`.

## Results

All four fresh parity preflights passed. Each target benchmark completed on its requested backend, terminal materialization completed, and fallback counts were empty. The actual loaded `_core.abi3.so` SHA-256 repeated across A legs (`04be3c7b596cfd411d6ce3588a6918185bd34c379dfee29650b30857d2d87a6c`) and across B legs (`bee589fe2e0f4ca1d61edcd208b28ad5757c0b4183a214c8dcc2110479158314`).

| Leg | Pillow median (ms) | CPU median (ms) | SIMD median (ms) | SIMD vector blocks | SIMD fused operations | SIMD host allocated bytes |
|---|---:|---:|---:|---:|---:|---:|
| A1 control | 2.023159 | 0.308367 | 2.835437 | 96 | 0 | 147,456 |
| B1 candidate | 2.315155 | 0.308771 | 0.484907 | 96 | 2 | 147,456 |
| B2 candidate | 2.031378 | 0.272826 | 0.480217 | 96 | 2 | 147,456 |
| A2 control | 2.052529 | 0.269096 | 2.817312 | 96 | 0 | 147,456 |

Control and candidate SIMD medians averaged across the two repeated legs are 2.826375 ms and 0.482562 ms, respectively: **5.856× faster with fusion**. Full-frame copy count was zero in every target leg. The corresponding CPU means across the repeated legs are 0.288731 ms (A) and 0.290799 ms (B), a 0.72% difference; this short trial does not show a material CPU regression.

Candidate SIMD measured 4.50× faster than same-run Pillow when comparing the mean of the two leg medians. This is below the repository's 5× SIMD target. Pillow-SIMD was not installed on this runner, so the matched Pillow-SIMD target remains unverified. The old published ratio and this Ryzen result must not be conflated.

## Files

Each leg retains its benchmark JSON, raw samples, benchmark execution attestation and children, and parity attestation. `experiment-metadata.json` records source/build identities and the A–B–B–A summary. These files were copied from the runner without copying its checkout, build directory, or virtual environment; nothing was deleted from the runner.
