//! RGBA mode strategies, including the guarded AVX2 transpose kernel.

use crate::compute::{Architecture, ExecutionContext, registry::support::{
    ImplementationSupport, InputContract, ModeSupport, OperationId, OutputModeRule,
    StrategyGuard, StrategyInputs, StrategySelection,
}};
use crate::error::PilError;
use crate::pipeline::TransposeMethod;
use crate::raster::{DynamicImage, Mode};
use crate::compute::pool_cpu::helpers::execution::prepared::Prepared;

const FLIP_INPUTS: &[InputContract] = &[];
const MIRROR_INPUTS: &[InputContract] = &[];
const TRANSPOSE_INPUTS: &[InputContract] = &[InputContract::CheckedByteDimensions {
    bytes_per_pixel: 4,
    allow_empty: true,
}];

const MIRROR_ARCHITECTURES: &[ImplementationSupport] = &[
    ImplementationSupport::cpu(
        OperationId::Mirror,
        Mode::ImageRgba8,
        Architecture::Sse2,
        cfg!(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2")),
        &[Architecture::Sse2],
        StrategyGuard::NativeMirrorRows { channels: 4 },
    ),
    ImplementationSupport::cpu(
        OperationId::Mirror,
        Mode::ImageRgba8,
        Architecture::Neon,
        cfg!(all(target_arch = "aarch64", target_feature = "neon")),
        &[Architecture::Neon],
        StrategyGuard::NativeMirrorRows { channels: 4 },
    ),
    ImplementationSupport::cpu(
        OperationId::Mirror,
        Mode::ImageRgba8,
        Architecture::WasmSimd128,
        cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
        &[Architecture::WasmSimd128],
        StrategyGuard::NativeMirrorRows { channels: 4 },
    ),
];

const TRANSPOSE_ARCHITECTURES: &[ImplementationSupport] = &[
    ImplementationSupport::cpu(
        OperationId::Transpose,
        Mode::ImageRgba8,
        Architecture::Avx2,
        cfg!(target_arch = "x86_64"),
        &[Architecture::Avx2],
        StrategyGuard::TransposeRgbaAvx2,
    ),
    ImplementationSupport::cpu(
        OperationId::Transpose,
        Mode::ImageRgba8,
        Architecture::Sse2,
        cfg!(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2")),
        &[Architecture::Sse2],
        StrategyGuard::TransposeNativeBytes { channels: 4 },
    ),
    ImplementationSupport::cpu(
        OperationId::Transpose,
        Mode::ImageRgba8,
        Architecture::Neon,
        cfg!(all(target_arch = "aarch64", target_feature = "neon")),
        &[Architecture::Neon],
        StrategyGuard::TransposeNativeBytes { channels: 4 },
    ),
    ImplementationSupport::cpu(
        OperationId::Transpose,
        Mode::ImageRgba8,
        Architecture::WasmSimd128,
        cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
        &[Architecture::WasmSimd128],
        StrategyGuard::TransposeNativeBytes { channels: 4 },
    ),
];

pub(crate) const FLIP_SUPPORT: ModeSupport =
    ModeSupport::serial(OperationId::Flip, Mode::ImageRgba8, FLIP_INPUTS);
pub(crate) const MIRROR_SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Mirror,
    Mode::ImageRgba8,
    OutputModeRule::Preserve,
    MIRROR_INPUTS,
    MIRROR_ARCHITECTURES,
);
pub(crate) const TRANSPOSE_SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Transpose,
    Mode::ImageRgba8,
    OutputModeRule::Preserve,
    TRANSPOSE_INPUTS,
    TRANSPOSE_ARCHITECTURES,
);

enum FlipArchitecture<'a> {
    Default(&'a super::Input),
}
enum MirrorArchitecture<'a> {
    Default(&'a super::Input),
    #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"))]
    Sse2(&'a super::Input),
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    Neon(&'a super::Input),
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    WasmSimd128(&'a super::Input),
}
enum TransposeArchitecture<'a> {
    Default(&'a super::Input, TransposeMethod),
    #[cfg(target_arch = "x86_64")]
    Avx2(&'a super::Input, TransposeMethod),
    #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"))]
    Sse2(&'a super::Input, TransposeMethod),
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    Neon(&'a super::Input, TransposeMethod),
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    WasmSimd128(&'a super::Input, TransposeMethod),
}

pub(crate) struct FlipStrategy<'a> {
    prepared: Prepared<FlipArchitecture<'a>>,
}
pub(crate) struct MirrorStrategy<'a> {
    prepared: Prepared<MirrorArchitecture<'a>>,
}
pub(crate) struct TransposeStrategy<'a> {
    prepared: Prepared<TransposeArchitecture<'a>>,
}

pub(crate) fn prepare_flip<'a>(
    image: &'a super::Input,
    ctx: &ExecutionContext,
) -> Result<FlipStrategy<'a>, PilError> {
    let selection = FLIP_SUPPORT.select_serial(ctx)?;
    Ok(FlipStrategy {
        prepared: Prepared::new(FlipArchitecture::Default(image), selection),
    })
}

pub(crate) fn prepare_mirror<'a>(
    image: &'a super::Input,
    ctx: &ExecutionContext,
) -> Result<MirrorStrategy<'a>, PilError> {
    let facts = StrategyInputs {
        width: image.width(),
        height: image.height(),
        byte_len: Some(image.as_raw().len()),
        palette: crate::compute::registry::support::PaletteKnowledge::Unknown,
        request: crate::compute::registry::support::RequestFacts::Unspecified,
    };
    let selection = MIRROR_SUPPORT.select(ctx, facts)?;
    let implementation = match selection.implementation.architecture {
        Architecture::Default => MirrorArchitecture::Default(image),
        #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"))]
        Architecture::Sse2 => MirrorArchitecture::Sse2(image),
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        Architecture::Neon => MirrorArchitecture::Neon(image),
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        Architecture::WasmSimd128 => MirrorArchitecture::WasmSimd128(image),
        _ => {
            return Err(PilError::InternalError(
                "unbound RGBA mirror architecture strategy".into(),
            ));
        }
    };
    Ok(MirrorStrategy {
        prepared: Prepared::new(implementation, selection),
    })
}

pub(crate) fn prepare_transpose<'a>(
    image: &'a super::Input,
    method: &TransposeMethod,
    mut facts: StrategyInputs,
    ctx: &ExecutionContext,
) -> Result<TransposeStrategy<'a>, PilError> {
    let (width, height) = image.dimensions();
    facts.width = width;
    facts.height = height;
    facts.byte_len = Some(image.as_raw().len());
    facts.request = crate::compute::registry::support::RequestFacts::Transpose {
        method: *method,
    };
    for contract in TRANSPOSE_INPUTS {
        contract.validate_materialized(facts)?;
    }
    let selection = TRANSPOSE_SUPPORT.select(ctx, facts)?;
    let method = *method;
    let implementation = match selection.implementation.architecture {
        #[cfg(target_arch = "x86_64")]
        Architecture::Avx2 => TransposeArchitecture::Avx2(image, method),
        #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"))]
        Architecture::Sse2 => TransposeArchitecture::Sse2(image, method),
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        Architecture::Neon => TransposeArchitecture::Neon(image, method),
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        Architecture::WasmSimd128 => TransposeArchitecture::WasmSimd128(image, method),
        Architecture::Default => TransposeArchitecture::Default(image, method),
        _ => return Err(PilError::InternalError(
            "unbound RGBA transpose architecture strategy".into(),
        )),
    };
    Ok(TransposeStrategy {
        prepared: Prepared::new(implementation, selection),
    })
}

impl FlipStrategy<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        self.prepared.selection()
    }

    pub(crate) fn execute(self) -> DynamicImage {
        match self.prepared.implementation {
            FlipArchitecture::Default(image) => super::default::flip_impl::execute(image),
        }
    }
}

impl MirrorStrategy<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        self.prepared.selection()
    }

    pub(crate) fn execute(self) -> DynamicImage {
        match self.prepared.implementation {
            MirrorArchitecture::Default(image) => super::default::mirror_impl::execute(image),
            #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"))]
            MirrorArchitecture::Sse2(image) => execute_native_mirror(image),
            #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
            MirrorArchitecture::Neon(image) => execute_native_mirror(image),
            #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
            MirrorArchitecture::WasmSimd128(image) => execute_native_mirror(image),
        }
    }
}

fn execute_native_mirror(image: &super::Input) -> DynamicImage {
    let Some((output, vector_blocks, scalar_tail)) =
        crate::compute::pool_cpu::ops::transpose::helpers::native_mirror::mirror_image(image, 4)
    else {
        return super::default::mirror_impl::execute(image);
    };
    crate::compute::record_pipeline_operation_path(if vector_blocks == 0 {
        "scalar-control"
    } else {
        "vector"
    });
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    DynamicImage::ImageRgba8(output)
}

impl TransposeStrategy<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        self.prepared.selection()
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        match self.prepared.implementation {
            TransposeArchitecture::Default(image, method) => {
                super::default::transpose_impl::execute(image, &method)
            }
            #[cfg(target_arch = "x86_64")]
            TransposeArchitecture::Avx2(image, method) => {
                // SAFETY: this variant is constructed only from the prepared
                // AVX2 selection, whose captured-capability and request guards
                // have already admitted the image and transpose method.
                unsafe { super::avx2::transpose_impl::execute(image, &method) }
            }
            #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"))]
            TransposeArchitecture::Sse2(image, method) => {
                super::sse2::execute(image, &method)
            }
            #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
            TransposeArchitecture::Neon(image, method) => {
                super::neon::execute(image, &method)
            }
            #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
            TransposeArchitecture::WasmSimd128(image, method) => {
                super::wasm_simd128::execute(image, &method)
            }
        }
    }
}
