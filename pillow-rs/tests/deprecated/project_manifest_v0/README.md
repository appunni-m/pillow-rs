# Deprecated project manifest v0

`manifest.yaml` is an exact snapshot of the project-wide Pillow 12.2.0
public-surface inventory from the repository root. It is migration evidence,
not an active parity manifest and not an output oracle. The root copy remains a
compatibility input for legacy coverage and benchmark tooling until those
consumers are migrated.

The maintained `migration-parity-fixtures` generator reads this inventory to
account for every legacy surface and public name in the single active manifest
at `pillow-rs/tests/fixtures/manifest.yaml`. Active parity runners never read
this directory.
