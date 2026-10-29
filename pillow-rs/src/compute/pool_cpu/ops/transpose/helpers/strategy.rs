//! Reusable mode-level strategy declaration for simple transpose operations.

macro_rules! declare_transpose_mode {
    ($mode:ident, $input:ty, $flip_impl:path, $mirror_impl:path, $transpose_impl:path) => {
        $crate::compute::pool_cpu::ops::transpose::helpers::strategy::declare_transpose_mode! {
            @declare $mode, $input, $flip_impl, $mirror_impl, $transpose_impl;
            &[]; [] []
        }
    };
    (
        $mode:ident, $input:ty, $flip_impl:path, $mirror_impl:path, $transpose_impl:path;
        mirror {
            $(
                $arch:ident, $cfg:meta, $requires:expr, $guard:expr
            );+ $(;)?
        }
    ) => {
        $crate::compute::pool_cpu::ops::transpose::helpers::strategy::declare_transpose_mode! {
            @declare $mode, $input, $flip_impl, $mirror_impl, $transpose_impl;
            &[]; [$(($arch, $cfg, $requires, $guard)),+] []
        }
    };
    (
        $mode:ident, $input:ty, $flip_impl:path, $mirror_impl:path, $transpose_impl:path;
        mirror {
            $(
                $march:ident, $mcfg:meta, $mrequires:expr, $mguard:expr
            );+ $(;)?
        }
        transpose {
            inputs: $transpose_inputs:expr;
            $(
                $tarch:ident, $tcfg:meta, $trequires:expr, $tguard:expr, $texecute:path
            );+ $(;)?
        }
    ) => {
        $crate::compute::pool_cpu::ops::transpose::helpers::strategy::declare_transpose_mode! {
            @declare $mode, $input, $flip_impl, $mirror_impl, $transpose_impl;
            $transpose_inputs;
            [$(($march, $mcfg, $mrequires, $mguard)),+]
            [$(($tarch, $tcfg, $trequires, $tguard, $texecute)),+]
        }
    };
    (
        @declare $mode:ident, $input:ty, $flip_impl:path, $mirror_impl:path,
        $transpose_impl:path; $transpose_inputs:expr;
        [$(($march:ident, $mcfg:meta, $mrequires:expr, $mguard:expr)),* $(,)?]
        [$(($tarch:ident, $tcfg:meta, $trequires:expr, $tguard:expr,
            $texecute:path)),* $(,)?]
    ) => {
        pub(crate) const FLIP_SUPPORT: crate::compute::registry::support::ModeSupport =
            crate::compute::registry::support::ModeSupport::serial(
                crate::compute::registry::support::OperationId::Flip,
                crate::raster::Mode::$mode,
                &[],
            );
        const MIRROR_ARCHITECTURES: &[
            crate::compute::registry::support::ImplementationSupport
        ] = &[
            $(crate::compute::registry::support::ImplementationSupport::cpu(
                crate::compute::registry::support::OperationId::Mirror,
                crate::raster::Mode::$mode,
                crate::compute::Architecture::$march,
                cfg!($mcfg),
                $mrequires,
                $mguard,
            ),)*
        ];
        pub(crate) const MIRROR_SUPPORT: crate::compute::registry::support::ModeSupport =
            crate::compute::registry::support::ModeSupport::cpu(
                crate::compute::registry::support::OperationId::Mirror,
                crate::raster::Mode::$mode,
                crate::compute::registry::support::OutputModeRule::Preserve,
                &[],
                MIRROR_ARCHITECTURES,
            );
        const TRANSPOSE_ARCHITECTURES: &[
            crate::compute::registry::support::ImplementationSupport
        ] = &[
            $(crate::compute::registry::support::ImplementationSupport::cpu(
                crate::compute::registry::support::OperationId::Transpose,
                crate::raster::Mode::$mode,
                crate::compute::Architecture::$tarch,
                cfg!($tcfg),
                $trequires,
                $tguard,
            ),)*
        ];
        pub(crate) const TRANSPOSE_SUPPORT: crate::compute::registry::support::ModeSupport =
            crate::compute::registry::support::ModeSupport::cpu(
                crate::compute::registry::support::OperationId::Transpose,
                crate::raster::Mode::$mode,
                crate::compute::registry::support::OutputModeRule::Preserve,
                $transpose_inputs,
                TRANSPOSE_ARCHITECTURES,
            );

        pub(crate) enum FlipArchitecture<'a> {
            Default(&'a $input),
        }
        pub(crate) enum MirrorArchitecture<'a> {
            Default(&'a $input),
            $(#[cfg($mcfg)] $march(&'a $input),)*
        }
        pub(crate) enum TransposeArchitecture<'a> {
            Default(&'a $input, crate::pipeline::TransposeMethod),
            $(#[cfg($tcfg)] $tarch(&'a $input, crate::pipeline::TransposeMethod),)*
        }
        pub(crate) struct FlipStrategy<'a> {
            prepared: crate::compute::pool_cpu::helpers::execution::prepared::Prepared<
                FlipArchitecture<'a>,
            >,
        }
        pub(crate) struct MirrorStrategy<'a> {
            prepared: crate::compute::pool_cpu::helpers::execution::prepared::Prepared<
                MirrorArchitecture<'a>,
            >,
        }
        pub(crate) struct TransposeStrategy<'a> {
            prepared: crate::compute::pool_cpu::helpers::execution::prepared::Prepared<
                TransposeArchitecture<'a>,
            >,
        }

        pub(crate) fn prepare_flip<'a>(
            image: &'a $input,
            ctx: &crate::compute::ExecutionContext,
        ) -> Result<FlipStrategy<'a>, crate::error::PilError> {
            let selection = FLIP_SUPPORT.select_serial(ctx)?;
            Ok(FlipStrategy {
                prepared: crate::compute::pool_cpu::helpers::execution::prepared::Prepared::new(
                    FlipArchitecture::Default(image),
                    selection,
                ),
            })
        }

        pub(crate) fn prepare_mirror<'a>(
            image: &'a $input,
            ctx: &crate::compute::ExecutionContext,
        ) -> Result<MirrorStrategy<'a>, crate::error::PilError> {
            let facts = crate::compute::registry::support::StrategyInputs {
                width: image.width(),
                height: image.height(),
                byte_len: Some(image.as_raw().len()),
                palette: crate::compute::registry::support::PaletteKnowledge::Unknown,
                request: crate::compute::registry::support::RequestFacts::Unspecified,
            };
            let selection = MIRROR_SUPPORT.select(ctx, facts)?;
            let implementation = match selection.implementation.architecture {
                crate::compute::Architecture::Default => MirrorArchitecture::Default(image),
                $(
                    #[cfg($mcfg)]
                    crate::compute::Architecture::$march => MirrorArchitecture::$march(image),
                )*
                _ => return Err(crate::error::PilError::InternalError(
                    "unbound mirror architecture strategy".into(),
                )),
            };
            Ok(MirrorStrategy {
                prepared: crate::compute::pool_cpu::helpers::execution::prepared::Prepared::new(
                    implementation,
                    selection,
                ),
            })
        }

        pub(crate) fn prepare_transpose<'a>(
            image: &'a $input,
            method: &crate::pipeline::TransposeMethod,
            facts: crate::compute::registry::support::StrategyInputs,
            ctx: &crate::compute::ExecutionContext,
        ) -> Result<TransposeStrategy<'a>, crate::error::PilError> {
            let selection = TRANSPOSE_SUPPORT.select(ctx, facts)?;
            let implementation = match selection.implementation.architecture {
                crate::compute::Architecture::Default => {
                    TransposeArchitecture::Default(image, *method)
                }
                $(
                    #[cfg($tcfg)]
                    crate::compute::Architecture::$tarch => {
                        TransposeArchitecture::$tarch(image, *method)
                    }
                )*
                _ => return Err(crate::error::PilError::InternalError(
                    "unbound transpose architecture strategy".into(),
                )),
            };
            Ok(TransposeStrategy {
                prepared: crate::compute::pool_cpu::helpers::execution::prepared::Prepared::new(
                    implementation,
                    selection,
                ),
            })
        }

        impl FlipStrategy<'_> {
            pub(crate) fn selection(
                &self,
            ) -> &crate::compute::registry::support::StrategySelection {
                self.prepared.selection()
            }

            pub(crate) fn execute(self) -> crate::raster::DynamicImage {
                match self.prepared.implementation {
                    FlipArchitecture::Default(image) => $flip_impl(image),
                }
            }
        }

        impl MirrorStrategy<'_> {
            pub(crate) fn selection(
                &self,
            ) -> &crate::compute::registry::support::StrategySelection {
                self.prepared.selection()
            }

            pub(crate) fn execute(self) -> crate::raster::DynamicImage {
                match self.prepared.implementation {
                    MirrorArchitecture::Default(image) => $mirror_impl(image),
                    $(
                        #[cfg($mcfg)]
                        MirrorArchitecture::$march(image) => execute_native_mirror(image),
                    )*
                }
            }
        }

        impl TransposeStrategy<'_> {
            pub(crate) fn selection(
                &self,
            ) -> &crate::compute::registry::support::StrategySelection {
                self.prepared.selection()
            }

            pub(crate) fn execute(
                self,
            ) -> Result<crate::raster::DynamicImage, crate::error::PilError> {
                match self.prepared.implementation {
                    TransposeArchitecture::Default(image, method) => {
                        $transpose_impl(image, &method)
                    },
                    $(
                        #[cfg($tcfg)]
                        TransposeArchitecture::$tarch(image, method) => $texecute(image, &method),
                    )*
                }
            }
        }

        #[cfg(any($($mcfg),*))]
        fn execute_native_mirror(image: &$input) -> crate::raster::DynamicImage {
            let channels = image.color().channel_count() as usize;
            let Some((output, vector_blocks, scalar_tail)) =
                $crate::compute::pool_cpu::ops::transpose::helpers::native_mirror::mirror_image(
                    image, channels,
                )
            else {
                return $mirror_impl(image);
            };
            $crate::compute::record_pipeline_operation_path(if vector_blocks == 0 {
                "scalar-control"
            } else {
                "vector"
            });
            $crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
            $crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
            crate::raster::DynamicImage::$mode(output)
        }
    };
}

macro_rules! declare_native_byte_transpose_mode {
    (
        $mode:ident, $input:ty, $flip_impl:path, $mirror_impl:path, $transpose_impl:path,
        $channels:literal
    ) => {
        $crate::compute::pool_cpu::ops::transpose::helpers::strategy::declare_transpose_mode! {
            $mode, $input, $flip_impl, $mirror_impl, $transpose_impl;
            mirror {
                Sse2,
                all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"),
                &[$crate::compute::Architecture::Sse2],
                $crate::compute::registry::support::StrategyGuard::NativeMirrorRows {
                    channels: $channels,
                };
                Neon,
                all(target_arch = "aarch64", target_feature = "neon"),
                &[$crate::compute::Architecture::Neon],
                $crate::compute::registry::support::StrategyGuard::NativeMirrorRows {
                    channels: $channels,
                };
                WasmSimd128,
                all(target_arch = "wasm32", target_feature = "simd128"),
                &[$crate::compute::Architecture::WasmSimd128],
                $crate::compute::registry::support::StrategyGuard::NativeMirrorRows {
                    channels: $channels,
                }
            }
            transpose {
                inputs: &[
                    $crate::compute::registry::support::InputContract::CheckedByteDimensions {
                        bytes_per_pixel: $channels,
                        allow_empty: true,
                    }
                ];
                Sse2,
                all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"),
                &[$crate::compute::Architecture::Sse2],
                $crate::compute::registry::support::StrategyGuard::TransposeNativeBytes {
                    channels: $channels,
                },
                super::sse2::execute;
                Neon,
                all(target_arch = "aarch64", target_feature = "neon"),
                &[$crate::compute::Architecture::Neon],
                $crate::compute::registry::support::StrategyGuard::TransposeNativeBytes {
                    channels: $channels,
                },
                super::neon::execute;
                WasmSimd128,
                all(target_arch = "wasm32", target_feature = "simd128"),
                &[$crate::compute::Architecture::WasmSimd128],
                $crate::compute::registry::support::StrategyGuard::TransposeNativeBytes {
                    channels: $channels,
                },
                super::wasm_simd128::execute
            }
        }
    };
}

macro_rules! declare_native_luma16_transpose_mode {
    ($mode:ident, $input:ty, $flip_impl:path, $mirror_impl:path, $transpose_impl:path) => {
        const FLIP_ARCHITECTURES: &[
            crate::compute::registry::support::ImplementationSupport
        ] = &[
            $crate::compute::registry::support::ImplementationSupport::cpu(
                crate::compute::registry::support::OperationId::Flip,
                crate::raster::Mode::$mode,
                crate::compute::Architecture::Sse2,
                cfg!(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2")),
                &[crate::compute::Architecture::Sse2],
                crate::compute::registry::support::StrategyGuard::ExactNativeRows { channels: 2 },
            ),
            $crate::compute::registry::support::ImplementationSupport::cpu(
                crate::compute::registry::support::OperationId::Flip,
                crate::raster::Mode::$mode,
                crate::compute::Architecture::Neon,
                cfg!(all(target_arch = "aarch64", target_feature = "neon")),
                &[crate::compute::Architecture::Neon],
                crate::compute::registry::support::StrategyGuard::ExactNativeRows { channels: 2 },
            ),
            $crate::compute::registry::support::ImplementationSupport::cpu(
                crate::compute::registry::support::OperationId::Flip,
                crate::raster::Mode::$mode,
                crate::compute::Architecture::WasmSimd128,
                cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
                &[crate::compute::Architecture::WasmSimd128],
                crate::compute::registry::support::StrategyGuard::ExactNativeRows { channels: 2 },
            ),
        ];
        const MIRROR_ARCHITECTURES: &[
            crate::compute::registry::support::ImplementationSupport
        ] = &[
            $crate::compute::registry::support::ImplementationSupport::cpu(
                crate::compute::registry::support::OperationId::Mirror,
                crate::raster::Mode::$mode,
                crate::compute::Architecture::Sse2,
                cfg!(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2")),
                &[crate::compute::Architecture::Sse2],
                crate::compute::registry::support::StrategyGuard::ExactNativeRows { channels: 2 },
            ),
            $crate::compute::registry::support::ImplementationSupport::cpu(
                crate::compute::registry::support::OperationId::Mirror,
                crate::raster::Mode::$mode,
                crate::compute::Architecture::Neon,
                cfg!(all(target_arch = "aarch64", target_feature = "neon")),
                &[crate::compute::Architecture::Neon],
                crate::compute::registry::support::StrategyGuard::ExactNativeRows { channels: 2 },
            ),
            $crate::compute::registry::support::ImplementationSupport::cpu(
                crate::compute::registry::support::OperationId::Mirror,
                crate::raster::Mode::$mode,
                crate::compute::Architecture::WasmSimd128,
                cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
                &[crate::compute::Architecture::WasmSimd128],
                crate::compute::registry::support::StrategyGuard::ExactNativeRows { channels: 2 },
            ),
        ];
        pub(crate) const FLIP_SUPPORT: crate::compute::registry::support::ModeSupport =
            crate::compute::registry::support::ModeSupport::cpu(
                crate::compute::registry::support::OperationId::Flip,
                crate::raster::Mode::$mode,
                crate::compute::registry::support::OutputModeRule::Preserve,
                &[],
                FLIP_ARCHITECTURES,
            );
        pub(crate) const MIRROR_SUPPORT: crate::compute::registry::support::ModeSupport =
            crate::compute::registry::support::ModeSupport::cpu(
                crate::compute::registry::support::OperationId::Mirror,
                crate::raster::Mode::$mode,
                crate::compute::registry::support::OutputModeRule::Preserve,
                &[],
                MIRROR_ARCHITECTURES,
            );
        const TRANSPOSE_ARCHITECTURES: &[
            crate::compute::registry::support::ImplementationSupport
        ] = &[
            $crate::compute::registry::support::ImplementationSupport::cpu(
                $crate::compute::registry::support::OperationId::Transpose,
                crate::raster::Mode::$mode,
                crate::compute::Architecture::Sse2,
                cfg!(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2")),
                &[crate::compute::Architecture::Sse2],
                crate::compute::registry::support::StrategyGuard::TransposeNativeBytes {
                    channels: 2,
                },
            ),
            $crate::compute::registry::support::ImplementationSupport::cpu(
                $crate::compute::registry::support::OperationId::Transpose,
                crate::raster::Mode::$mode,
                crate::compute::Architecture::Neon,
                cfg!(all(target_arch = "aarch64", target_feature = "neon")),
                &[crate::compute::Architecture::Neon],
                crate::compute::registry::support::StrategyGuard::TransposeNativeBytes {
                    channels: 2,
                },
            ),
            $crate::compute::registry::support::ImplementationSupport::cpu(
                $crate::compute::registry::support::OperationId::Transpose,
                crate::raster::Mode::$mode,
                crate::compute::Architecture::WasmSimd128,
                cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
                &[crate::compute::Architecture::WasmSimd128],
                crate::compute::registry::support::StrategyGuard::TransposeNativeBytes {
                    channels: 2,
                },
            ),
        ];
        const TRANSPOSE_INPUTS: &[crate::compute::registry::support::InputContract] = &[
            crate::compute::registry::support::InputContract::CheckedByteDimensions {
                bytes_per_pixel: 2,
                allow_empty: true,
            },
        ];
        pub(crate) const TRANSPOSE_SUPPORT: crate::compute::registry::support::ModeSupport =
            crate::compute::registry::support::ModeSupport::cpu(
                crate::compute::registry::support::OperationId::Transpose,
                crate::raster::Mode::$mode,
                crate::compute::registry::support::OutputModeRule::Preserve,
                TRANSPOSE_INPUTS,
                TRANSPOSE_ARCHITECTURES,
            );

        enum FlipArchitecture<'a> {
            Default(&'a $input),
            #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"))]
            Sse2(&'a $input),
            #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
            Neon(&'a $input),
            #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
            WasmSimd128(&'a $input),
        }
        enum MirrorArchitecture<'a> {
            Default(&'a $input),
            #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"))]
            Sse2(&'a $input),
            #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
            Neon(&'a $input),
            #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
            WasmSimd128(&'a $input),
        }
        enum TransposeArchitecture<'a> {
            Default(&'a $input, crate::pipeline::TransposeMethod),
            #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"))]
            Sse2(&'a $input, crate::pipeline::TransposeMethod),
            #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
            Neon(&'a $input, crate::pipeline::TransposeMethod),
            #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
            WasmSimd128(&'a $input, crate::pipeline::TransposeMethod),
        }
        pub(crate) struct FlipStrategy<'a> {
            prepared: crate::compute::pool_cpu::helpers::execution::prepared::Prepared<
                FlipArchitecture<'a>,
            >,
        }
        pub(crate) struct MirrorStrategy<'a> {
            prepared: crate::compute::pool_cpu::helpers::execution::prepared::Prepared<
                MirrorArchitecture<'a>,
            >,
        }
        pub(crate) struct TransposeStrategy<'a> {
            prepared: crate::compute::pool_cpu::helpers::execution::prepared::Prepared<
                TransposeArchitecture<'a>,
            >,
        }

        fn native_luma16_facts(
            image: &$input,
        ) -> crate::compute::registry::support::StrategyInputs {
            crate::compute::registry::support::StrategyInputs {
                width: image.width(),
                height: image.height(),
                byte_len: Some(
                    image.as_raw().len().saturating_mul(std::mem::size_of::<u16>()),
                ),
                palette: crate::compute::registry::support::PaletteKnowledge::Unknown,
                request: crate::compute::registry::support::RequestFacts::Unspecified,
            }
        }

        pub(crate) fn prepare_flip<'a>(
            image: &'a $input,
            ctx: &crate::compute::ExecutionContext,
        ) -> Result<FlipStrategy<'a>, crate::error::PilError> {
            let selection = FLIP_SUPPORT.select(ctx, native_luma16_facts(image))?;
            let implementation = match selection.implementation.architecture {
                crate::compute::Architecture::Default => FlipArchitecture::Default(image),
                #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"))]
                crate::compute::Architecture::Sse2 => FlipArchitecture::Sse2(image),
                #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
                crate::compute::Architecture::Neon => FlipArchitecture::Neon(image),
                #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
                crate::compute::Architecture::WasmSimd128 => FlipArchitecture::WasmSimd128(image),
                _ => return Err(crate::error::PilError::InternalError(
                    "unbound I;16 flip architecture strategy".into(),
                )),
            };
            Ok(FlipStrategy {
                prepared: crate::compute::pool_cpu::helpers::execution::prepared::Prepared::new(
                    implementation,
                    selection,
                ),
            })
        }

        pub(crate) fn prepare_mirror<'a>(
            image: &'a $input,
            ctx: &crate::compute::ExecutionContext,
        ) -> Result<MirrorStrategy<'a>, crate::error::PilError> {
            let selection = MIRROR_SUPPORT.select(ctx, native_luma16_facts(image))?;
            let implementation = match selection.implementation.architecture {
                crate::compute::Architecture::Default => MirrorArchitecture::Default(image),
                #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"))]
                crate::compute::Architecture::Sse2 => MirrorArchitecture::Sse2(image),
                #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
                crate::compute::Architecture::Neon => MirrorArchitecture::Neon(image),
                #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
                crate::compute::Architecture::WasmSimd128 => MirrorArchitecture::WasmSimd128(image),
                _ => return Err(crate::error::PilError::InternalError(
                    "unbound I;16 mirror architecture strategy".into(),
                )),
            };
            Ok(MirrorStrategy {
                prepared: crate::compute::pool_cpu::helpers::execution::prepared::Prepared::new(
                    implementation,
                    selection,
                ),
            })
        }

        pub(crate) fn prepare_transpose<'a>(
            image: &'a $input,
            method: &crate::pipeline::TransposeMethod,
            facts: crate::compute::registry::support::StrategyInputs,
            ctx: &crate::compute::ExecutionContext,
        ) -> Result<TransposeStrategy<'a>, crate::error::PilError> {
            let selection = TRANSPOSE_SUPPORT.select(ctx, facts)?;
            let implementation = match selection.implementation.architecture {
                crate::compute::Architecture::Default => {
                    TransposeArchitecture::Default(image, *method)
                }
                #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"))]
                crate::compute::Architecture::Sse2 => {
                    TransposeArchitecture::Sse2(image, *method)
                }
                #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
                crate::compute::Architecture::Neon => {
                    TransposeArchitecture::Neon(image, *method)
                }
                #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
                crate::compute::Architecture::WasmSimd128 => {
                    TransposeArchitecture::WasmSimd128(image, *method)
                }
                _ => return Err(crate::error::PilError::InternalError(
                    "unbound I;16 transpose architecture strategy".into(),
                )),
            };
            Ok(TransposeStrategy {
                prepared: crate::compute::pool_cpu::helpers::execution::prepared::Prepared::new(
                    implementation,
                    selection,
                ),
            })
        }

        impl FlipStrategy<'_> {
            pub(crate) fn selection(
                &self,
            ) -> &crate::compute::registry::support::StrategySelection {
                self.prepared.selection()
            }

            pub(crate) fn execute(self) -> crate::raster::DynamicImage {
                match self.prepared.implementation {
                    FlipArchitecture::Default(image) => $flip_impl(image),
                    #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"))]
                    FlipArchitecture::Sse2(image) => execute_native_flip(image),
                    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
                    FlipArchitecture::Neon(image) => execute_native_flip(image),
                    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
                    FlipArchitecture::WasmSimd128(image) => execute_native_flip(image),
                }
            }
        }

        impl MirrorStrategy<'_> {
            pub(crate) fn selection(
                &self,
            ) -> &crate::compute::registry::support::StrategySelection {
                self.prepared.selection()
            }

            pub(crate) fn execute(self) -> crate::raster::DynamicImage {
                match self.prepared.implementation {
                    MirrorArchitecture::Default(image) => $mirror_impl(image),
                    #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"))]
                    MirrorArchitecture::Sse2(image) => execute_native_mirror(image),
                    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
                    MirrorArchitecture::Neon(image) => execute_native_mirror(image),
                    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
                    MirrorArchitecture::WasmSimd128(image) => execute_native_mirror(image),
                }
            }
        }

        impl TransposeStrategy<'_> {
            pub(crate) fn selection(
                &self,
            ) -> &crate::compute::registry::support::StrategySelection {
                self.prepared.selection()
            }

            pub(crate) fn execute(
                self,
            ) -> Result<crate::raster::DynamicImage, crate::error::PilError> {
                match self.prepared.implementation {
                    TransposeArchitecture::Default(image, method) => $transpose_impl(image, &method),
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

        #[cfg(any(
            all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"),
            all(target_arch = "aarch64", target_feature = "neon"),
            all(target_arch = "wasm32", target_feature = "simd128")
        ))]
        fn execute_native_flip(image: &$input) -> crate::raster::DynamicImage {
            let Some((output, vector_blocks, scalar_tail)) =
                $crate::compute::pool_cpu::ops::transpose::helpers::native_luma16::flip_image(image)
            else {
                return $flip_impl(image);
            };
            $crate::compute::record_pipeline_operation_path("native-copy");
            $crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
            $crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
            crate::raster::DynamicImage::$mode(output)
        }

        #[cfg(any(
            all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"),
            all(target_arch = "aarch64", target_feature = "neon"),
            all(target_arch = "wasm32", target_feature = "simd128")
        ))]
        fn execute_native_mirror(image: &$input) -> crate::raster::DynamicImage {
            let Some((output, vector_blocks, scalar_tail)) =
                $crate::compute::pool_cpu::ops::transpose::helpers::native_luma16::mirror_image(image)
            else {
                return $mirror_impl(image);
            };
            $crate::compute::record_pipeline_operation_path("native-copy");
            $crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
            $crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
            crate::raster::DynamicImage::$mode(output)
        }
    };
}

pub(crate) use declare_transpose_mode;
pub(crate) use declare_native_byte_transpose_mode;
pub(crate) use declare_native_luma16_transpose_mode;
