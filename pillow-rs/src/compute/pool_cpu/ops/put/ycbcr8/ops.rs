//! `ImageYCbCr8` band semantics and context-selected architecture strategies.

use crate::compute::registry::support::BandIndexRule;
use super::super::helpers::extract_band_strategy::declare_strategies;

declare_strategies! {
    ImageYCbCr8, BandIndexRule::RgbWithOpaqueAlpha;
    Ssse3, ssse3, any(target_arch = "x86_64", all(target_arch = "x86", target_feature = "ssse3")),
        &[crate::compute::Architecture::Sse2, crate::compute::Architecture::Ssse3], 3, unsafe;
    Sse2, sse2, all(target_arch = "x86", target_feature = "sse2", not(target_feature = "ssse3")),
        &[crate::compute::Architecture::Sse2], 3, safe;
    WasmSimd128, wasm_simd128, all(target_arch = "wasm32", target_feature = "simd128"),
        &[crate::compute::Architecture::WasmSimd128], 3, safe;
    Neon, neon, all(target_arch = "aarch64", target_feature = "neon"),
        &[crate::compute::Architecture::Neon], 3, safe;

}

use super::super::helpers::put_pixel_strategy::declare_strategies as declare_pixel_strategies;

declare_pixel_strategies! {
    ImageYCbCr8, 3;
    single [
        Sse2, sse2, all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"), &[crate::compute::Architecture::Sse2];
        WasmSimd128, wasm_simd128, all(target_arch = "wasm32", target_feature = "simd128"), &[crate::compute::Architecture::WasmSimd128];
        Neon, neon, all(target_arch = "aarch64", target_feature = "neon"), &[crate::compute::Architecture::Neon];
    ]
    batch [
    ]
}

use super::super::helpers::put_data_strategy::declare_strategies as declare_data_strategies;

declare_data_strategies! {
    ImageYCbCr8, 3, super::super::helpers::put_data::THREE_BYTE_PAYLOADS;
    Sse2, sse2, all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"), &[crate::compute::Architecture::Sse2];
    WasmSimd128, wasm_simd128, all(target_arch = "wasm32", target_feature = "simd128"), &[crate::compute::Architecture::WasmSimd128];
    Neon, neon, all(target_arch = "aarch64", target_feature = "neon"), &[crate::compute::Architecture::Neon];
}

use super::super::helpers::put_alpha_strategy::declare_strategies as declare_alpha_strategies;

declare_alpha_strategies! {
    ImageYCbCr8, ImageRgba8, 3, 4;
}

use super::super::helpers::put_alpha_data_strategy::declare_strategies as declare_alpha_data_strategies;

declare_alpha_data_strategies! {
    ImageYCbCr8, ImageRgba8, 0, 4;
}
