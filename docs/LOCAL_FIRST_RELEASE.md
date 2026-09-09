# Local first-release receipt

This receipt records the coordinated local bootstrap prepared on 2026-09-10.
It is a file-backed rehearsal for the first versions of the three projects; it
does not publish to a public registry or write to a remote Git repository.

## Release identities

| Project | Version | Annotated tag | Exact commit |
| --- | --- | --- | --- |
| `image-slash-star` | `0.1.0` | `v0.1.0` | `7a53a343d7217ba40448c9b97af0afbd81a09793` |
| `fontdone` | `2.14.3-alpha.1` | `v2.14.3-alpha.1` | `df7ac49b8df3f8f8be96fdb6d1a5c2588f840f74` |
| `pillow-rs` | `0.1.0` | `v0.1.0` | `003830605ea81640366988a477c19dd7793d9eb1` |

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

The staged Cargo source registry follows the dependency order used by the
public release workflows:

1. `image-slash-star-0.1.0.crate`
2. `fontdone-2.14.3-alpha.1.crate`
3. `fontdone-c-abi-2.14.3-alpha.1.crate`
4. `fontdone-wasm-2.14.3-alpha.1.crate`
5. the registry-normalized `pillow-rs-0.1.0.crate`

The same bundle contains the `pillow-rs` ABI3 wheel and the `pillow-rs` and
`fontdone` npm tarballs. A local Cargo consumer was resolved offline with Rust
1.96.1, including the C-ABI and raw-WASM facade crates. Fresh Python wheel and
npm consumers also imported the packaged artifacts successfully. Image package
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
  `image-slash-star`, `fontdone`, `fontdone-c-abi`, `fontdone-wasm`, and
  `pillow-rs`;
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
