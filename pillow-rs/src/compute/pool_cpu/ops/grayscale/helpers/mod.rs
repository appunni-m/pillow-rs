pub(crate) mod luma;
#[cfg(any(
    all(
        any(target_arch = "x86", target_arch = "x86_64"),
        any(target_feature = "sse2", target_feature = "ssse3", target_feature = "avx2"),
    ),
    all(target_arch = "wasm32", target_feature = "simd128"),
    all(target_arch = "aarch64", target_feature = "neon"),
))]
pub(crate) mod interleaved;
#[cfg(any(
    all(
        any(target_arch = "x86", target_arch = "x86_64"),
        any(target_feature = "sse2", target_feature = "ssse3", target_feature = "avx2"),
    ),
    all(target_arch = "wasm32", target_feature = "simd128"),
))]
pub(crate) mod cmyk;
