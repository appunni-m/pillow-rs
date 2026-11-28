//! Scalar pixel transforms shared by exact-mode implementations.

/// Inverts every stored byte, preserving the original native-byte behavior.
// TODO(pillow-rs#3ptb): freeze raw-suffix behavior for Invert. Default leaves
// currently transform the full backing slice; vector leaves transform only
// the checked width × height × channel prefix and preserve any trailing bytes.
// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
pub(crate) fn invert_bytes(source: &[u8]) -> Vec<u8> {
    source.iter().map(|value| u8::MAX - *value).collect()
}
