# Quarantined x86_64 reports

These first x86_64 reports are retained for audit history and must not be
counted as performance evidence. Candidate source files were copied with
`cp -p`, preserving modification times older than Cargo's existing target
artifact. Cargo reused the control library; runtime attestations show the same
core hash, `9d2d192f…`, for both purported A and B. The `ssse3-gather` receipt
therefore proves only that the control binary ran that path, not that the
candidate was built.

After forcing source timestamps newer than the cached artifact, Cargo compiled
the candidate and exposed an x86-only unused `channels` parameter under the
warning-as-error build. The parameter was renamed `_channels`; the AArch64
NEON use remains unchanged. The corrected x86_64 A–B–B–A reports are in the
parent `x86/` directory and bind distinct control and candidate binary hashes.

No files were removed from the native runner. Its scratch directory and source
backups were retained.
