mod extract_band;
#[cfg(any(
    all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"),
    all(target_arch = "wasm32", target_feature = "simd128"),
    all(target_arch = "aarch64", target_feature = "neon"),
))]
mod put_pixel;
