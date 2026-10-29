pub(crate) mod extract_band;
pub(crate) mod extract_band_strategy;
pub(crate) mod put_pixel;
pub(crate) mod put_pixel_strategy;
pub(crate) mod put_data;
pub(crate) mod put_data_strategy;
pub(crate) mod put_alpha;
pub(crate) mod put_alpha_strategy;
pub(crate) mod alpha_mask;
pub(crate) mod put_alpha_data;
pub(crate) mod put_alpha_data_strategy;
#[cfg(any(
    all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"),
    all(target_arch = "wasm32", target_feature = "simd128"),
    all(target_arch = "aarch64", target_feature = "neon"),
))]
pub(crate) mod put_data_vector;
#[cfg(any(
    all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"),
    all(target_arch = "wasm32", target_feature = "simd128"),
    all(target_arch = "aarch64", target_feature = "neon"),
))]
pub(crate) mod masked_store;
#[cfg(any(
    all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"),
    all(target_arch = "wasm32", target_feature = "simd128"),
    all(target_arch = "aarch64", target_feature = "neon"),
))]
pub(crate) mod put_pixel_vector;
#[cfg(any(
    target_arch = "x86_64",
    all(target_arch = "x86", any(target_feature = "sse2", target_feature = "ssse3")),
    all(target_arch = "wasm32", target_feature = "simd128"),
    all(target_arch = "aarch64", target_feature = "neon"),
))]
pub(crate) mod extract_band_vector;
#[cfg(any(
    all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"),
    all(target_arch = "wasm32", target_feature = "simd128"),
    all(target_arch = "aarch64", target_feature = "neon"),
))]
pub(crate) mod put_alpha_vector;
#[cfg(any(
    all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"),
    all(target_arch = "wasm32", target_feature = "simd128"),
    all(target_arch = "aarch64", target_feature = "neon"),
))]
pub(crate) mod put_alpha_data_vector;
