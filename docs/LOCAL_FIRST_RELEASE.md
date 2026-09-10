# Local first-release receipt

This receipt records the coordinated local bootstrap prepared on 2026-09-10.
It is a file-backed rehearsal for the first versions of the three projects; it
does not publish to a public registry or write to a remote Git repository.

## Release identities

| Project | Version | Annotated tag | Exact commit |
| --- | --- | --- | --- |
| `image-slash-star` | `0.1.0` | `v0.1.0` | `8d8ecdfe8699329ae166541be8040b1bc3b253e7` |
| `fontdone` | `2.14.3-alpha.2` | `v2.14.3-alpha.2` | `ec58b10c29fae9b9ca4c622568bc1410596608fd` |
| `pillow-rs` | `0.1.0` | `v0.1.0` | see `dist/release-local/release-manifest.txt` |

The tags and complete Git bundles are under `dist/release-local/` in the
working checkout. The bundle's `SHA256SUMS` is the authority for every package
archive and Git bundle; verify it from that directory with:

```sh
shasum -a 256 -c SHA256SUMS
shasum -a 256 -c benchmarks/SHA256SUMS
```

The maintained Makefile check validates the same receipt, both checksum
manifests, every package entry, all three Git bundles, and the Cargo release
identity: each public archive must match its local-registry copy, index
checksum, tagged VCS revision, and embedded dependency checksums:

```sh
make release-local-check
```

## Local publication order

The staged local artifacts follow the dependency order used by the public
release workflows:

1. `image-slash-star-0.1.0.crate`
2. the public `fontdone-2.14.3-alpha.2.crate`
3. the native `fontdone-c-abi-2.14.3-alpha.2-aarch64-apple-darwin.tar.gz` SDK
4. the `fontdone@2.14.3-alpha.2` browser npm archive
5. the registry-normalized `pillow-rs-0.1.0.crate`

The same bundle contains the `pillow-rs` ABI3 wheel, the native fontdone C SDK
archive, and the `pillow-rs` and `fontdone` npm tarballs. A local Cargo
consumer was resolved offline with Rust 1.96.1 against the public `fontdone`
crate and `image-slash-star`; the internal C and raw-WASM facade archives were
also compiled by the fontdone package audit. Fresh Python wheel and npm
consumers imported the packaged artifacts successfully. Image package
verification and the fontdone release dry-run passed from the exact tagged
checkouts.

The local `fontdone@2.14.3-alpha.2` tarball is the synchronized candidate
built from the pushed fontdone `main` commit. The earlier npm
`2.14.3-alpha.1` archive remains immutable historical evidence and is not
used by the current release bundle.

The local Cargo registry is intentionally a read-only source registry for
consumer verification, not a fake crates.io upload endpoint. Python and npm
are likewise verified by installing the exact local wheel and tarballs. The
full command log and artifact list are retained in
`dist/release-local/LOCAL_RELEASE_RECEIPT.txt`.

## Benchmark receipt

The bundle retains a clean tagged fixed-cohort pair under
`dist/release-local/benchmarks/`. Both runs use the tagged commit, the same
manifest/input hashes, and each workload's declared repeat policy. The retained
11-workload cohort uses one warmup, three measurement iterations, and two
samples (six timed executions per subject). Native CPU, SIMD, and GPU terminal
receipts make 44/44 rows comparable; the adjacent budget check reports nine
timing-only violations. A newer host-access run at the current root revision
completed five identical fixed-cohort runs; its adjacent comparisons reported
3, 7, 5, and 5 timing-only violations. A further current-HEAD pair at
`de571ae57` retained the same receipt invariants and reported 12 timing-only
violations. All sets remain release evidence and leave the zero-violation
acceptance item open; no threshold, workload ID, fixture, or receipt rule was
changed.

## Public-release prerequisites

The rehearsal itself did not publish to crates.io, PyPI, npm, create a GitHub
release, or push a release tag. After it was assembled, the clean `fontdone`
`main` branch was pushed at `ec58b10c29fae9b9ca4c622568bc1410596608fd`, and
the clean pillow-rs release-prep branch is present on `origin`. The
image-slash-star `main` push
was rejected by GitHub because its earlier history contains generated AV1
blobs above the hosting limit; its verified clean-history candidate is
`codex/release-image-slash-star-clean` at
`8d8ecdfe8699329ae166541be8040b1bc3b253e7`. Before enabling public registry
publication, the owner must provide or configure:

- crates.io ownership or the `crates-io` Trusted Publisher for
  `image-slash-star`, `fontdone`, and `pillow-rs`;
- PyPI ownership or the `pypi` Trusted Publisher for `pillow-rs`;
- npm ownership, two-factor authentication, and the `npm` Trusted Publisher
  for `fontdone` and `pillow-rs`; and
- GitHub push access plus protected `crates-io`, `pypi`, and `npm` environments.

The public release gates remain visible rather than being bypassed: the image
project still has an incomplete strict LLVM source-coverage denominator,
fontdone still has unresolved C-ABI route/error debt and lacks five fresh
cross-platform C-ABI bundles including the Windows import library, and the root benchmark checklist still requires two
consecutive fixed-ID comparisons with zero timing-budget violations. The image
repository now has a pushed clean-history candidate at
`codex/release-image-slash-star-clean`; updating `main` still requires explicit
authorization because it rewrites historical Git objects.

After those prerequisites are satisfied, the release workflows publish only
from a clean reviewed commit and create later releases from an immutable
annotated `v<version>` tag.
