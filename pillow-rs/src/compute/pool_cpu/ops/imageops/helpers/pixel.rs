//! Scalar pixel transforms shared by exact-mode implementations.

/// Inverts every stored byte, preserving the original native-byte behavior.
pub(crate) fn invert_bytes(source: &[u8]) -> Vec<u8> {
    source.iter().map(|value| u8::MAX - *value).collect()
}
