//! Composition helpers for exact-mode imageops strategies.

use crate::compute::{Architecture, Backend, ExecutionContext};
use crate::compute::registry::support::{
    ImplementationSupport, ModeSupport, OperationId, PaletteKnowledge,
    RequestFacts, SecondaryInputFacts, StrategyGuard, StrategyInputs, StrategySelection,
    SupportAssessment,
};
use crate::error::PilError;
use crate::image::Image;
use crate::raster::{DynamicImage, Mode};
use std::sync::Arc;

pub(crate) fn selected_quantize_architecture(
    selection: &StrategySelection,
    mode: Mode,
) -> Result<Architecture, PilError> {
    let implementation = selection.implementation;
    if implementation.operation == OperationId::Quantize
        && implementation.mode == mode
        && implementation.backend == Backend::Cpu
    {
        return Ok(implementation.architecture);
    }
    Err(PilError::InternalError(
        "Quantize mode received a mismatched CPU strategy selection".into(),
    ))
}

macro_rules! declare_quantize_native_mode {
    ($component:ty, $mode_name:literal, $mode:ident) => {
        pub(crate) fn execute_quantize(
            samples: &[$component],
            width: u32,
            height: u32,
            colors: usize,
            kmeans: u32,
            method: u32,
            selection: &$crate::compute::registry::support::StrategySelection,
        ) -> Result<(Vec<u8>, Vec<u8>, Option<Vec<u8>>), $crate::error::PilError> {
            let architecture = super::super::helpers::strategy::selected_quantize_architecture(
                selection,
                $crate::raster::Mode::$mode,
            )?;
            match architecture {
                $crate::compute::Architecture::Default => {
                    super::default::quantize_impl::execute(
                        samples, width, height, colors, kmeans, method,
                    )
                }
                _ => Err($crate::error::PilError::InternalError(
                    concat!($mode_name, " Quantize selected an unimplemented architecture")
                        .into(),
                )),
            }
        }
    };
}

macro_rules! declare_quantize_native_default {
    ($component:ty, $channels:literal) => {
        pub(crate) fn execute(
            samples: &[$component],
            width: u32,
            height: u32,
            colors: usize,
            kmeans: u32,
            method: u32,
        ) -> Result<(Vec<u8>, Vec<u8>, Option<Vec<u8>>), $crate::error::PilError> {
            $crate::compute::pool_cpu::ops::imageops::helpers::quantize_rgb_source::execute_interleaved::<
                $component,
                $channels,
            >(samples, width, height, colors, kmeans, method)
        }
    };
    (rgba_interleaved, $component:ty, $channels:literal) => {
        pub(crate) fn execute(
            samples: &[$component],
            width: u32,
            height: u32,
            colors: usize,
            _kmeans: u32,
            _method: u32,
        ) -> Result<(Vec<u8>, Vec<u8>, Option<Vec<u8>>), $crate::error::PilError> {
            $crate::compute::pool_cpu::ops::imageops::helpers::quantize_rgb_source::execute_rgba_interleaved::<
                $component,
                $channels,
            >(samples, width, height, colors)
        }
    };
    (premultiplied_luma_alpha) => {
        pub(crate) fn execute(
            samples: &[u8],
            width: u32,
            height: u32,
            colors: usize,
            _kmeans: u32,
            _method: u32,
        ) -> Result<(Vec<u8>, Vec<u8>, Option<Vec<u8>>), $crate::error::PilError> {
            $crate::compute::pool_cpu::ops::imageops::helpers::quantize_rgb_source::execute_premultiplied_luma_alpha(
                samples, width, height, colors,
            )
        }
    };
    (premultiplied_rgba) => {
        pub(crate) fn execute(
            samples: &[u8],
            width: u32,
            height: u32,
            colors: usize,
            _kmeans: u32,
            _method: u32,
        ) -> Result<(Vec<u8>, Vec<u8>, Option<Vec<u8>>), $crate::error::PilError> {
            $crate::compute::pool_cpu::ops::imageops::helpers::quantize_rgb_source::execute_premultiplied_rgba(
                samples, width, height, colors,
            )
        }
    };
    (palette_alpha) => {
        pub(crate) fn execute(
            samples: &[u8],
            palette: $crate::compute::registry::support::PaletteInput<'_>,
            width: u32,
            height: u32,
            colors: usize,
        ) -> Result<(Vec<u8>, Vec<u8>, Option<Vec<u8>>), $crate::error::PilError> {
            $crate::compute::pool_cpu::ops::imageops::helpers::quantize_rgb_source::execute_palette_alpha(
                samples, palette, width, height, colors,
            )
        }
    };
    (clipped_luma16) => {
        pub(crate) fn execute(
            samples: &[u16],
            width: u32,
            height: u32,
            colors: usize,
            kmeans: u32,
            method: u32,
        ) -> Result<(Vec<u8>, Vec<u8>, Option<Vec<u8>>), $crate::error::PilError> {
            $crate::compute::pool_cpu::ops::imageops::helpers::quantize_rgb_source::execute_clipped_luma16(
                samples, width, height, colors, kmeans, method,
            )
        }
    };
    (i32_scalar) => {
        pub(crate) fn execute(
            samples: &[u8],
            width: u32,
            height: u32,
            colors: usize,
            kmeans: u32,
            method: u32,
        ) -> Result<(Vec<u8>, Vec<u8>, Option<Vec<u8>>), $crate::error::PilError> {
            $crate::compute::pool_cpu::ops::imageops::helpers::quantize_rgb_source::execute_i32_scalar(
                samples, width, height, colors, kmeans, method,
            )
        }
    };
    (f32_scalar) => {
        pub(crate) fn execute(
            samples: &[u8],
            width: u32,
            height: u32,
            colors: usize,
            kmeans: u32,
            method: u32,
        ) -> Result<(Vec<u8>, Vec<u8>, Option<Vec<u8>>), $crate::error::PilError> {
            $crate::compute::pool_cpu::ops::imageops::helpers::quantize_rgb_source::execute_f32_scalar(
                samples, width, height, colors, kmeans, method,
            )
        }
    };
    (rgba) => {
        pub(crate) fn execute(
            samples: &[u8],
            width: u32,
            height: u32,
            colors: usize,
            _kmeans: u32,
            _method: u32,
        ) -> Result<(Vec<u8>, Vec<u8>, Option<Vec<u8>>), $crate::error::PilError> {
            $crate::compute::pool_cpu::ops::imageops::helpers::quantize_rgb_source::execute_rgba_samples(
                samples, width, height, colors,
            )
        }
    };
}

pub(crate) use declare_quantize_native_default;
pub(crate) use declare_quantize_native_mode;

pub(crate) fn imageops_mask_facts(
    mask: Option<&Arc<Image>>,
) -> Result<Option<SecondaryInputFacts>, PilError> {
    let Some(mask) = mask else {
        return Ok(None);
    };
    let mode = match mask.known_mode_identity() {
        Some(mode) => mode,
        None => mask.materialized_shared()?.mode(),
    };
    let (width, height) = mask.size()?;
    Ok(Some(SecondaryInputFacts {
        mode,
        width,
        height,
        byte_len: None,
    }))
}

pub(crate) const fn native_byte_point_architectures(
    operation: OperationId,
    mode: Mode,
    channels: u8,
) -> [ImplementationSupport; 3] {
    cpu_architectures_for(operation, mode, StrategyGuard::NativePointwiseBytes { channels })
}

pub(crate) const fn cpu_architectures_for(
    operation: OperationId,
    mode: Mode,
    guard: StrategyGuard,
) -> [ImplementationSupport; 3] {
    [
        ImplementationSupport::cpu(
            operation,
            mode,
            Architecture::Sse2,
            cfg!(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "sse2"
            )),
            &[Architecture::Sse2],
            guard,
        ),
        ImplementationSupport::cpu(
            operation,
            mode,
            Architecture::Neon,
            cfg!(all(target_arch = "aarch64", target_feature = "neon")),
            &[Architecture::Neon],
            guard,
        ),
        ImplementationSupport::cpu(
            operation,
            mode,
            Architecture::WasmSimd128,
            cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
            &[Architecture::WasmSimd128],
            guard,
        ),
    ]
}

macro_rules! declare_expand_byte_mode {
    ($mode:ident, $channels:literal) => {
        declare_expand_byte_mode!(
            @impl
            $mode,
            $channels,
            crate::compute::registry::support::ExpandFillRule::ModeEncodedBytes
        );
    };
    ($mode:ident, $channels:literal, $fill_rule:expr $(,)?) => {
        declare_expand_byte_mode!(@impl $mode, $channels, $fill_rule);
    };
    (@impl $mode:ident, $channels:literal, $fill_rule:expr) => {
        use crate::compute::{
            Architecture, ExecutionContext,
            pool_cpu::helpers::execution::prepared::Prepared,
        };
        use crate::compute::registry::support::{
            ImplementationSupport, InputContract, ModeSupport, OperationId, OutputModeRule,
            PaletteKnowledge, StrategyGuard, StrategyInputs,
        };
        use crate::error::PilError;
        use crate::raster::{DynamicImage, Mode};

        const INPUTS: &[InputContract] = &[
            InputContract::ExpandedDimensions {
                bytes_per_pixel: $channels,
            },
            InputContract::ExpandFill($fill_rule),
        ];
        static ARCHITECTURES: [ImplementationSupport; 3] =
            super::super::helpers::strategy::cpu_architectures_for(
                OperationId::Expand,
                Mode::$mode,
                StrategyGuard::ExpandVector { channels: $channels },
            );
        pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
            OperationId::Expand,
            Mode::$mode,
            OutputModeRule::Preserve,
            INPUTS,
            &ARCHITECTURES,
        );

        pub(crate) enum ArchitectureStrategy<'a> {
            Default(default::Kernel<'a>),
            Vector {
                input: &'a super::Input,
                border: crate::pipeline::ExpandMargins,
                fill: (u8, u8, u8, u8),
            },
        }

        pub(crate) struct Strategy<'a> {
            prepared: Prepared<ArchitectureStrategy<'a>>,
            input: &'a super::Input,
            border: crate::pipeline::ExpandMargins,
            fill: (u8, u8, u8, u8),
        }

        pub(crate) fn prepare<'a>(
            input: &'a super::Input,
            border: crate::pipeline::ExpandMargins,
            fill: (u8, u8, u8, u8),
            ctx: &ExecutionContext,
        ) -> Result<Strategy<'a>, PilError> {
            let facts = StrategyInputs {
                width: input.width(),
                height: input.height(),
                byte_len: Some(std::mem::size_of_val(input.as_raw().as_slice())),
                palette: PaletteKnowledge::Unknown,
                request: Default::default(),
            }
            .with_expand_border(border);
            let selection = super::super::helpers::strategy::select_expand(SUPPORT, facts, ctx)?;
            let implementation = match selection.implementation.architecture {
                Architecture::Default => {
                    ArchitectureStrategy::Default(default::prepare(input, border, fill))
                }
                Architecture::Sse2 | Architecture::Neon | Architecture::WasmSimd128 => {
                    ArchitectureStrategy::Vector { input, border, fill }
                }
                _ => {
                    return Err(PilError::InternalError(
                        "Expand selected an undeclared architecture".into(),
                    ));
                }
            };
            Ok(Strategy {
                prepared: Prepared::new(implementation, selection),
                input,
                border,
                fill,
            })
        }

        impl Strategy<'_> {
            pub(crate) fn selection(
                &self,
            ) -> &crate::compute::registry::support::StrategySelection {
                self.prepared.selection()
            }
        }

        pub(crate) fn execute(strategy: Strategy<'_>) -> Result<DynamicImage, PilError> {
            let Strategy { prepared, input, border, fill } = strategy;
            let output = match prepared.implementation {
                ArchitectureStrategy::Default(kernel) => {
                    default::execute(kernel)?
                }
                ArchitectureStrategy::Vector { input, border, fill } => {
                    let (output, vector_blocks, scalar_tail) =
                        super::super::helpers::expand_vector::expand_buffer(
                            input, border, $channels, fill,
                        )?;
                    crate::compute::record_pipeline_operation_path("native-copy");
                    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
                    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
                    output
                }
            };
            let mut image = DynamicImage::$mode(output);
            super::super::helpers::hidden_lanes::compose_expand_lanes(
                Mode::$mode,
                input,
                &mut image,
                border,
                fill,
            )?;
            Ok(image)
        }
    };
}

pub(crate) use declare_expand_byte_mode;

pub(crate) const fn unsupported_mode_reason(mode: Mode) -> &'static str {
    match mode {
        Mode::One => "not supported for mode 1",
        Mode::ImageLuma8 => "not supported for mode L",
        Mode::ImageLumaA8 => "not supported for mode LA",
        Mode::ImageLaPremultiplied8 => "not supported for mode La",
        Mode::ImagePalette8 => "not supported for mode P",
        Mode::ImagePaletteAlpha8 => "not supported for mode PA",
        Mode::ImageRgb8 => "not supported for mode RGB",
        Mode::ImageRgba8 => "not supported for mode RGBA",
        Mode::ImageRgbPremultiplied8 => "not supported for mode RGBa",
        Mode::ImageRgbx8 => "not supported for mode RGBX",
        Mode::ImageCmyk8 => "not supported for mode CMYK",
        Mode::ImageYCbCr8 => "not supported for mode YCbCr",
        Mode::ImageLab8 => "not supported for mode LAB",
        Mode::ImageHsv8 => "not supported for mode HSV",
        Mode::ImageI32 => "not supported for mode I",
        Mode::ImageF32 => "not supported for mode F",
        Mode::ImageI16 => "not supported for mode I;16",
        Mode::ImageI16L => "not supported for mode I;16L",
        Mode::ImageI16B => "not supported for mode I;16B",
        Mode::ImageI16N => "not supported for mode I;16N",
        Mode::ImageLumaA16 => "not supported for mode LA",
        Mode::ImageRgb16 | Mode::ImageRgb32F => "not supported for mode RGB",
        Mode::ImageRgba16 | Mode::ImageRgba32F => "not supported for mode RGBA",
    }
}

pub(crate) fn select_mode(
    support: ModeSupport,
    image: &DynamicImage,
    request: RequestFacts,
    palette: PaletteKnowledge,
    ctx: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    let inputs = StrategyInputs {
        width: image.width(),
        height: image.height(),
        byte_len: Some(image.as_bytes().len()),
        palette,
        request,
    };
    match support.assess(ctx, inputs)? {
        SupportAssessment::Ready(selection) => Ok(selection),
        SupportAssessment::Rejected { reason, kind, .. } => Err(kind.into_error(reason)),
        SupportAssessment::Deferred { .. } => Err(PilError::InternalError(
            "ImageOps preparation has unresolved input facts".into(),
        )),
    }
}

pub(crate) fn select_expand(
    support: ModeSupport,
    input: StrategyInputs,
    ctx: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    match support.assess(ctx, input)? {
        SupportAssessment::Ready(selection) => Ok(selection),
        SupportAssessment::Rejected { reason, kind, .. } => Err(kind.into_error(reason)),
        SupportAssessment::Deferred { .. } => Err(PilError::InternalError(
            "Expand preparation has unresolved input facts".into(),
        )),
    }
}

pub(crate) fn select_invert_strategy(
    support: crate::compute::registry::support::ModeSupport,
    width: u32,
    height: u32,
    byte_len: usize,
    ctx: &crate::compute::ExecutionContext,
) -> Result<crate::compute::registry::support::StrategySelection, crate::error::PilError> {
    use crate::compute::registry::support::{
        PaletteKnowledge, StrategyInputs, SupportAssessment,
    };
    match support.assess(ctx, StrategyInputs {
        width,
        height,
        byte_len: Some(byte_len),
        palette: PaletteKnowledge::Unknown,
        request: Default::default(),
    })? {
        SupportAssessment::Ready(selection) => Ok(selection),
        SupportAssessment::Rejected { reason, kind, .. } => Err(kind.into_error(reason)),
        SupportAssessment::Deferred { .. } => Err(crate::error::PilError::InternalError(
            "Invert preparation requires materialized input facts".into(),
        )),
    }
}

macro_rules! declare_invert_mode {
    ($mode:ident, $input:ty, $implementation:path, $bytes_per_pixel:expr) => {
        declare_invert_mode!(
            $mode,
            $input,
            $implementation,
            $bytes_per_pixel,
            None::<&'static str>
        );
    };
    ($mode:ident, $input:ty, $implementation:path, $bytes_per_pixel:expr, $core_only_reason:expr) => {
        pub(crate) const SUPPORT: crate::compute::registry::support::ModeSupport =
            match $core_only_reason {
                Some(reason) => crate::compute::registry::support::ModeSupport::cpu_core_only(
                    crate::compute::registry::support::OperationId::Invert,
                    crate::raster::Mode::$mode,
                    crate::compute::registry::support::OutputModeRule::Preserve,
                    &[crate::compute::registry::support::InputContract::CheckedByteDimensions {
                        bytes_per_pixel: $bytes_per_pixel,
                        allow_empty: true,
                    }],
                    &[],
                    reason,
                ),
                None => crate::compute::registry::support::ModeSupport::cpu(
                    crate::compute::registry::support::OperationId::Invert,
                    crate::raster::Mode::$mode,
                    crate::compute::registry::support::OutputModeRule::Preserve,
                    &[crate::compute::registry::support::InputContract::CheckedByteDimensions {
                        bytes_per_pixel: $bytes_per_pixel,
                        allow_empty: true,
                    }],
                    &[],
                ),
            };

        pub(crate) enum ArchitectureStrategy<'a> {
            Default(&'a $input),
        }

        pub(crate) type Strategy<'a> = crate::compute::pool_cpu::helpers::execution::prepared::Prepared<
            ArchitectureStrategy<'a>,
        >;

        pub(crate) fn prepare<'a>(
            input: &'a $input,
            ctx: &crate::compute::ExecutionContext,
        ) -> Result<Strategy<'a>, crate::error::PilError> {
            let selection = super::super::helpers::strategy::select_invert_strategy(
                SUPPORT,
                input.width(),
                input.height(),
                input.as_raw().len(),
                ctx,
            )?;
            Ok(Strategy::new(ArchitectureStrategy::Default(input), selection))
        }

        pub(crate) fn execute(
            strategy: Strategy<'_>,
        ) -> Result<crate::raster::DynamicImage, crate::error::PilError> {
                match strategy.implementation {
                    ArchitectureStrategy::Default(input) => $implementation(input),
                }
        }
    };
}

macro_rules! declare_invert_native_mode {
    ($mode:ident, $input:ty, $channels:expr, $bytes_per_pixel:expr $(,)?) => {
        declare_invert_native_mode!(
            $mode,
            $input,
            $channels,
            $bytes_per_pixel,
            None
        );
    };
    ($mode:ident, $input:ty, $channels:expr, $bytes_per_pixel:expr, $core_only_reason:expr $(,)?) => {
        const ARCHITECTURES: [crate::compute::registry::support::ImplementationSupport; 3] = [
            crate::compute::registry::support::ImplementationSupport::cpu(
                crate::compute::registry::support::OperationId::Invert,
                crate::raster::Mode::$mode,
                crate::compute::Architecture::Sse2,
                cfg!(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2")),
                &[crate::compute::Architecture::Sse2],
                crate::compute::registry::support::StrategyGuard::NativeBytePrefix {
                    channels: $channels,
                },
            ),
            crate::compute::registry::support::ImplementationSupport::cpu(
                crate::compute::registry::support::OperationId::Invert,
                crate::raster::Mode::$mode,
                crate::compute::Architecture::Neon,
                cfg!(all(target_arch = "aarch64", target_feature = "neon")),
                &[crate::compute::Architecture::Neon],
                crate::compute::registry::support::StrategyGuard::NativeBytePrefix {
                    channels: $channels,
                },
            ),
            crate::compute::registry::support::ImplementationSupport::cpu(
                crate::compute::registry::support::OperationId::Invert,
                crate::raster::Mode::$mode,
                crate::compute::Architecture::WasmSimd128,
                cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
                &[crate::compute::Architecture::WasmSimd128],
                crate::compute::registry::support::StrategyGuard::NativeBytePrefix {
                    channels: $channels,
                },
            ),
        ];

        // NO_PILLOW_SUPPORT: `Some(reason)` declares the exact mode's native Invert extension.
        pub(crate) const SUPPORT: crate::compute::registry::support::ModeSupport = match $core_only_reason {
            Some(reason) => crate::compute::registry::support::ModeSupport::cpu_core_only(
                crate::compute::registry::support::OperationId::Invert,
                crate::raster::Mode::$mode,
                crate::compute::registry::support::OutputModeRule::Preserve,
                &[crate::compute::registry::support::InputContract::CheckedByteDimensions {
                    bytes_per_pixel: $bytes_per_pixel,
                    allow_empty: true,
                }],
                &ARCHITECTURES,
                reason,
            ),
            None => crate::compute::registry::support::ModeSupport::cpu(
                crate::compute::registry::support::OperationId::Invert,
                crate::raster::Mode::$mode,
                crate::compute::registry::support::OutputModeRule::Preserve,
                &[crate::compute::registry::support::InputContract::CheckedByteDimensions {
                    bytes_per_pixel: $bytes_per_pixel,
                    allow_empty: true,
                }],
                &ARCHITECTURES,
            ),
        };

        pub(crate) enum ArchitectureStrategy<'a> {
            Default(&'a $input),
            Sse2(&'a $input),
            Neon(&'a $input),
            WasmSimd128(&'a $input),
        }

        pub(crate) struct Strategy<'a> {
            prepared: crate::compute::pool_cpu::helpers::execution::prepared::Prepared<
                ArchitectureStrategy<'a>,
            >,
        }

        pub(crate) fn prepare<'a>(
            input: &'a $input,
            ctx: &crate::compute::ExecutionContext,
        ) -> Result<Strategy<'a>, crate::error::PilError> {
            let selection = super::super::helpers::strategy::select_invert_strategy(
                SUPPORT,
                input.width(),
                input.height(),
                input.as_raw().len(),
                ctx,
            )?;
            let implementation = match selection.implementation.architecture {
                crate::compute::Architecture::Sse2 => ArchitectureStrategy::Sse2(input),
                crate::compute::Architecture::Neon => ArchitectureStrategy::Neon(input),
                crate::compute::Architecture::WasmSimd128 =>
                    ArchitectureStrategy::WasmSimd128(input),
                _ => ArchitectureStrategy::Default(input),
            };
            Ok(Strategy {
                prepared: crate::compute::pool_cpu::helpers::execution::prepared::Prepared::new(
                    implementation,
                    selection,
                ),
            })
        }

        impl Strategy<'_> {
            pub(crate) fn selection(
                &self,
            ) -> &crate::compute::registry::support::StrategySelection {
                self.prepared.selection()
            }

            pub(crate) fn execute(
                self,
            ) -> Result<crate::raster::DynamicImage, crate::error::PilError> {
                match self.prepared.implementation {
                    ArchitectureStrategy::Default(input) => super::default::invert_impl::execute(input),
                    ArchitectureStrategy::Sse2(input) => super::sse2::invert_impl::execute(input),
                    ArchitectureStrategy::Neon(input) => super::neon::invert_impl::execute(input),
                    ArchitectureStrategy::WasmSimd128(input) =>
                        super::wasm_simd128::invert_impl::execute(input),
                }
            }
        }

        pub(crate) fn execute(
            strategy: Strategy<'_>,
        ) -> Result<crate::raster::DynamicImage, crate::error::PilError> {
            strategy.execute()
        }
    };
}

macro_rules! declare_invert_architecture {
    ($mode:ident, $carrier_mode:ident, $input:ty, $channels:expr, $invert_alpha:expr) => {
        pub(crate) fn execute(input: &$input) -> Result<crate::raster::DynamicImage, crate::error::PilError> {
            use crate::compute::pool_cpu::ops::imageops::helpers::native_bytes::NativeInvertOutput;
            let (width, height) = input.dimensions();
            match crate::compute::pool_cpu::ops::imageops::helpers::native_bytes::invert(
                input.as_raw(), width, height, $channels, $invert_alpha,
            )? {
                NativeInvertOutput::Empty => {
                    crate::compute::record_pipeline_operation_path("native-copy");
                    Ok(crate::raster::DynamicImage::$mode(input.clone()))
                }
                NativeInvertOutput::Exact(bytes) => {
                    let image = crate::image_utils::raw_bytes_to_image(
                        width, height, bytes, $channels,
                    )?;
                    let crate::raster::DynamicImage::$carrier_mode(image) = image else {
                        return Err(crate::error::PilError::InternalError(
                            "Invert kernel returned a different carrier layout".into(),
                        ));
                    };
                    crate::compute::record_pipeline_operation_path("vector");
                    Ok(crate::raster::DynamicImage::$mode(image))
                }
                NativeInvertOutput::Prefix(bytes) => {
                    let image = <$input>::from_raw(width, height, bytes).ok_or_else(|| {
                        crate::error::PilError::InternalError(
                            "Invert kernel returned an invalid byte prefix".into(),
                        )
                    })?;
                    crate::compute::record_pipeline_operation_path("vector");
                    Ok(crate::raster::DynamicImage::$mode(image))
                }
            }
        }
    };
}

pub(crate) use declare_invert_architecture;
pub(crate) use declare_invert_mode;
pub(crate) use declare_invert_native_mode;
