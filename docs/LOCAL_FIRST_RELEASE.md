# Local first-release receipt

This receipt records the coordinated local bootstrap prepared on 2026-09-10.
It is a file-backed rehearsal for the first versions of the three projects; it
does not publish to a public registry or write to a remote Git repository.

## Release identities

| Project | Version | Annotated tag | Exact commit |
| --- | --- | --- | --- |
| `image-slash-star` | `0.1.0` | `v0.1.0` | `35dd72808e6b2a8488b98caf685a3d48e4c97468` |
| `fontdone` | `2.14.3-alpha.1` | `v2.14.3-alpha.1` | `ad5e771aa0621d6e699260d23fbbb35e2a081e82` |
| `pillow-rs` | `0.1.0` | `v0.1.0` | `38e7281954154f3cc8cd62767b462b55a4475003` |

The tags and complete Git bundles are under `dist/release-local/` in the
working checkout. The bundle's `SHA256SUMS` is the authority for every package
archive and Git bundle; verify it from that directory with:

```sh
shasum -a 256 -c SHA256SUMS
shasum -a 256 -c benchmarks/SHA256SUMS
```

The maintained Makefile check validates the same receipt, both checksum
manifests, every package entry, and all three Git bundles:

```sh
make release-local-check
```

## Local publication order

The staged local artifacts follow the dependency order used by the public
release workflows:

1. `image-slash-star-0.1.0.crate`
2. the public `fontdone-2.14.3-alpha.1.crate`
3. the native `fontdone-c-abi-2.14.3-alpha.1-aarch64-apple-darwin.tar.gz` SDK
4. the `fontdone@2.14.3-alpha.1` browser npm archive
5. the registry-normalized `pillow-rs-0.1.0.crate`

The same bundle contains the `pillow-rs` ABI3 wheel, the native fontdone C SDK
archive, and the `pillow-rs` and `fontdone` npm tarballs. A local Cargo
consumer was resolved offline with Rust 1.96.1 against the public `fontdone`
crate and `image-slash-star`; the internal C and raw-WASM facade archives were
also compiled by the fontdone package audit. Fresh Python wheel and npm
consumers imported the packaged artifacts successfully. Image package
verification and the fontdone release dry-run passed from the exact tagged
checkouts.

The local Cargo registry is intentionally a read-only source registry for
consumer verification, not a fake crates.io upload endpoint. Python and npm
are likewise verified by installing the exact local wheel and tarballs. The
full command log and artifact list are retained in
`dist/release-local/LOCAL_RELEASE_RECEIPT.txt`.

## Benchmark receipt

The bundle also retains a fresh clean fixed-cohort pair under
`dist/release-local/benchmarks/`. Both runs use the tagged commit, the same
manifest/input hashes, and the unchanged five-warmup/20-iteration/five-sample
policy. Native CPU, SIMD, and GPU terminal receipts make 44/44 rows comparable;
the adjacent budget check reports nine timing-only violations. The result is
kept as release evidence and leaves the zero-violation acceptance item open;
no threshold, workload ID, fixture, or receipt rule was changed.

## Public-release prerequisites

No crates.io, PyPI, npm, GitHub release, or remote Git operation was performed.
Before enabling public publication, the owner must provide or configure:

- crates.io ownership or the `crates-io` Trusted Publisher for
  `image-slash-star`, `fontdone`, and `pillow-rs`;
- PyPI ownership or the `pypi` Trusted Publisher for `pillow-rs`;
- npm ownership, two-factor authentication, and the `npm` Trusted Publisher
  for `fontdone` and `pillow-rs`; and
- GitHub push access plus protected `crates-io`, `pypi`, and `npm` environments.

The public release gates remain visible rather than being bypassed: the image
project still has an incomplete strict LLVM source-coverage denominator,
fontdone still lacks five fresh cross-platform C-ABI bundles including the
Windows import library, and the root benchmark checklist still requires two
consecutive fixed-ID comparisons with zero timing-budget violations.

After those prerequisites are satisfied, the release workflows publish only
from a clean reviewed commit and create later releases from an immutable
annotated `v<version>` tag.
