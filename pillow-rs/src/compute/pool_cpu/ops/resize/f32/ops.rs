//! Exact ImageF32 mode dispatch for ordinary and boxed Resize.
//!
//! Prepared selection chooses the exact mode and architecture strategy. The
//! portable Default sequence uses a fixed 16-product grouping; x86_64 keeps
//! source-ordered separate arithmetic, AArch64 keeps its 4-product grouping,
//! and vector leaves own their ISA route.

use crate::compute::registry::support::{
    ImplementationSupport, ModeSupport, OperationId, OutputModeRule, StrategyGuard,
    StrategySelection,
};
use crate::compute::{Architecture, ExecutionContext};
use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::{DynamicImage, Mode, RgbaImage};

// TODO(pillow-rs#3ptb): Compile the declared x86, x86_64, AArch64, and Wasm
// strategy configurations; verify F32 Resize pixels, selected/fallback
// receipts, and per-architecture latency in the frozen CPU campaign.
static RESIZE_ARCHITECTURES: [ImplementationSupport; 9] = [
    ImplementationSupport::cpu(
        OperationId::Resize,
        Mode::ImageF32,
        Architecture::Avx512F,
        cfg!(any(target_arch = "x86", target_arch = "x86_64")),
        &[Architecture::Avx512F],
        StrategyGuard::ResizeAvx512Nearest,
    ),
    ImplementationSupport::cpu(
        OperationId::Resize,
        Mode::ImageF32,
        Architecture::Avx512F,
        cfg!(any(target_arch = "x86", target_arch = "x86_64")),
        &[Architecture::Avx512F],
        StrategyGuard::ResizeFilteredAvx512F,
    ),
    ImplementationSupport::cpu(
        OperationId::Resize,
        Mode::ImageF32,
        Architecture::AvxFma,
        cfg!(target_arch = "x86_64"),
        &[Architecture::AvxFma],
        StrategyGuard::ResizeFilteredFma,
    ),
    ImplementationSupport::cpu(
        OperationId::Resize,
        Mode::ImageF32,
        Architecture::Avx,
        cfg!(all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "avx",
            not(target_feature = "fma")
        )),
        &[Architecture::Avx],
        StrategyGuard::ResizeF32Vector,
    ),
    ImplementationSupport::cpu(
        OperationId::Resize,
        Mode::ImageF32,
        Architecture::Sse2,
        cfg!(all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "sse2",
            not(target_feature = "avx"),
            not(target_feature = "fma")
        )),
        &[Architecture::Sse2],
        StrategyGuard::ResizeF32Vector,
    ),
    ImplementationSupport::cpu(
        OperationId::Resize,
        Mode::ImageF32,
        Architecture::Neon,
        cfg!(all(target_arch = "aarch64", target_feature = "neon")),
        &[Architecture::Neon],
        StrategyGuard::ResizeF32Vector,
    ),
    ImplementationSupport::cpu(
        OperationId::Resize,
        Mode::ImageF32,
        Architecture::WasmSimd128,
        cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
        &[Architecture::WasmSimd128],
        StrategyGuard::ResizeF32Vector,
    ),
    ImplementationSupport::cpu(
        OperationId::Resize,
        Mode::ImageF32,
        Architecture::Aarch64,
        cfg!(target_arch = "aarch64"),
        &[],
        StrategyGuard::Always,
    ),
    ImplementationSupport::cpu(
        OperationId::Resize,
        Mode::ImageF32,
        Architecture::X86_64,
        cfg!(target_arch = "x86_64"),
        &[],
        StrategyGuard::Always,
    ),
];

static RESIZE_BOXED_ARCHITECTURES: [ImplementationSupport; 7] = [
    ImplementationSupport::cpu(
        OperationId::ResizeBoxed,
        Mode::ImageF32,
        Architecture::AvxFma,
        cfg!(target_arch = "x86_64"),
        &[Architecture::AvxFma],
        StrategyGuard::ResizeFilteredFma,
    ),
    ImplementationSupport::cpu(
        OperationId::ResizeBoxed,
        Mode::ImageF32,
        Architecture::Avx,
        cfg!(all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "avx",
            not(target_feature = "fma")
        )),
        &[Architecture::Avx],
        StrategyGuard::ResizeF32Vector,
    ),
    ImplementationSupport::cpu(
        OperationId::ResizeBoxed,
        Mode::ImageF32,
        Architecture::Sse2,
        cfg!(all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "sse2",
            not(target_feature = "avx"),
            not(target_feature = "fma")
        )),
        &[Architecture::Sse2],
        StrategyGuard::ResizeF32Vector,
    ),
    ImplementationSupport::cpu(
        OperationId::ResizeBoxed,
        Mode::ImageF32,
        Architecture::Neon,
        cfg!(all(target_arch = "aarch64", target_feature = "neon")),
        &[Architecture::Neon],
        StrategyGuard::ResizeF32Vector,
    ),
    ImplementationSupport::cpu(
        OperationId::ResizeBoxed,
        Mode::ImageF32,
        Architecture::WasmSimd128,
        cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
        &[Architecture::WasmSimd128],
        StrategyGuard::ResizeF32Vector,
    ),
    ImplementationSupport::cpu(
        OperationId::ResizeBoxed,
        Mode::ImageF32,
        Architecture::Aarch64,
        cfg!(target_arch = "aarch64"),
        &[],
        StrategyGuard::Always,
    ),
    ImplementationSupport::cpu(
        OperationId::ResizeBoxed,
        Mode::ImageF32,
        Architecture::X86_64,
        cfg!(target_arch = "x86_64"),
        &[],
        StrategyGuard::Always,
    ),
];

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Resize,
    Mode::ImageF32,
    OutputModeRule::Preserve,
    super::super::helpers::support::INPUTS,
    &RESIZE_ARCHITECTURES,
);
pub(crate) const BOXED_SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::ResizeBoxed,
    Mode::ImageF32,
    OutputModeRule::Preserve,
    super::super::helpers::support::INPUTS,
    &RESIZE_BOXED_ARCHITECTURES,
);

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn resize(
    context: &ExecutionContext,
    image: &DynamicImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: super::super::helpers::typed_nearest::BoxBounds,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageF32(input) = image else {
        return Err(PilError::InternalError(
            "ImageF32 Resize strategy received another raster mode".into(),
        ));
    };

    let output = match bounds {
        Some((left, top, right, bottom))
            if u64::from(image.height()) > u64::from(image.width()) * 100
                && height < image.height() =>
        {
            let vertical = resize_selected(
                context,
                selection,
                input,
                image.width(),
                height,
                filter,
                Some((0.0, top, f64::from(image.width()), bottom)),
            )?;
            resize_selected(
                context,
                selection,
                &vertical,
                width,
                height,
                filter,
                Some((left, 0.0, right, f64::from(height))),
            )?
        }
        _ => resize_selected(context, selection, input, width, height, filter, bounds)?,
    };
    Ok(DynamicImage::ImageF32(output))
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn resize_selected(
    context: &ExecutionContext,
    selection: &StrategySelection,
    input: &RgbaImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: super::super::helpers::typed_nearest::BoxBounds,
) -> Result<RgbaImage, PilError> {
    let architecture = selection.implementation.architecture;
    match architecture {
        Architecture::Default => {
            super::default::resize_impl::execute(input, width, height, filter, bounds)
        }
        #[cfg(target_arch = "x86_64")]
        Architecture::X86_64 => super::x86_64::resize_impl::execute(
            context, input, width, height, filter, bounds, selection,
        ),
        #[cfg(target_arch = "aarch64")]
        Architecture::Aarch64 => super::aarch64::resize_impl::execute(
            context, input, width, height, filter, bounds, selection,
        ),
        #[cfg(target_arch = "x86_64")]
        Architecture::AvxFma => super::avx::resize_impl::execute(
            context, input, width, height, filter, bounds, selection,
        ),
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        Architecture::Avx512F => super::avx512f::resize_impl::execute(
            context, input, width, height, filter, bounds, selection,
        ),
        #[cfg(all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "avx",
            not(target_feature = "fma")
        ))]
        Architecture::Avx => super::avx::vector_resize_impl::execute(
            context, input, width, height, filter, bounds, selection,
        ),
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        Architecture::Sse2 => super::sse2::resize_impl::execute(
            context, input, width, height, filter, bounds, selection,
        ),
        #[cfg(target_arch = "aarch64")]
        Architecture::Neon => super::neon::resize_impl::execute(
            context, input, width, height, filter, bounds, selection,
        ),
        #[cfg(target_arch = "wasm32")]
        Architecture::WasmSimd128 => super::wasm_simd128::resize_impl::execute(
            context, input, width, height, filter, bounds, selection,
        ),
        _ => Err(PilError::InternalError(format!(
            "ImageF32 Resize received non-F32 implementation selection {architecture:?}"
        ))),
    }
}
