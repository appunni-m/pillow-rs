# Releasing pillow-rs

`pillow-rs` is released as three coordinated artifacts from one reviewed
commit:

| Artifact | Package | Registry | Prerequisite |
| --- | --- | --- | --- |
| Rust core | `pillow-rs` | crates.io | `image-slash-star` and `fontdone` are visible at their pinned versions |
| Python binding | `pillow-rs` | PyPI | the Rust package is available and the wheel consumer check passes |
| WebAssembly binding | `pillow-rs` | npm | the Rust package is available and the npm consumer check passes |

The C SDK and the `fontdone` browser package are released from the separate
`fontdone` repository. Its workspace contains private Cargo build packages;
only its root `fontdone` package is published to crates.io.

## Bootstrap rehearsal

From a clean checkout, run the maintained checks before any upload:

```sh
make release-local-check
MIGRATION_WASM_NO_OPT=1 make release-check RELEASE_CRATES_READY=0
```

The first command verifies the file-backed bundle, checksums, package archives,
and Git bundles. The second command runs the registry-independent root release
gate. Keep parity, coverage, benchmark, and backend fallback evidence attached
to the exact commit; do not change a fixture or threshold to make a release
pass.

Publish the dependency crates first, then rerun the root gate with
`RELEASE_CRATES_READY=1`. The first public upload requires the package owners'
registry credentials or trusted publishers for crates.io, PyPI, and npm. The
release workflows use short-lived OIDC credentials behind protected GitHub
environments; no registry token belongs in the repository.

## Subsequent releases

After the bootstrap, update all authoritative versions and changelog entries,
run the full release gate, and push one annotated immutable `v<version>` tag.
The tag workflow verifies that the tag, package metadata, lockfile, and hosted
CI run identify the same commit before publishing crates.io, PyPI, and npm
artifacts. Never move a tag after a registry upload.
