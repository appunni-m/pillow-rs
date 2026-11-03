//! Exact-mode native-byte filter leaves.

/// Copy a native filter identity through the shared vector-copy path and
/// publish the same operation receipt as the archived architecture adapter.
pub(crate) fn execute_native_filter_identity_copy(
    raw: &[u8],
) -> Result<Vec<u8>, crate::error::PilError> {
    let mut output = vec![0; raw.len()];
    let (vector_blocks, scalar_tail) =
        crate::compute::pool_cpu::helpers::bytes::vector_copy::copy_native_bytes(raw, &mut output)
            .ok_or_else(|| {
                crate::error::PilError::InternalError(
                    "native filter identity copy buffer mismatch".into(),
                )
            })?;
    crate::compute::record_pipeline_operation_path("native-copy");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    Ok(output)
}

macro_rules! native_filter_mode_leaf {
    (
        $variant:ident,
        $carrier:ident,
        $channels:expr,
        $uniform_shortcut:expr,
        $la_special:expr,
        $filter5x5_impl:path
    ) => {
        pub(crate) fn execute(
            image: &crate::raster::DynamicImage,
            request: $crate::compute::pool_cpu::ops::filter::ops::FilterRequest,
        ) -> Result<crate::raster::DynamicImage, crate::error::PilError> {
            use $crate::compute::pool_cpu::ops::filter::helpers::{
                execute_filter3x3_la_binomial_bytes, execute_filter3x3_native_bytes,
                execute_native_bytes_filter,
            };

            match request {
                $crate::compute::pool_cpu::ops::filter::ops::FilterRequest::Filter3x3 {
                    kernel,
                    scale,
                    offset,
                } => {
                    let crate::raster::DynamicImage::$variant(input) = image else {
                        return Err($crate::error::PilError::InternalError(
                            "filter mode strategy received a mismatched raster variant".into(),
                        ));
                    };
                    let output = if $la_special {
                        execute_filter3x3_la_binomial_bytes(
                            input.as_raw(),
                            input.width(),
                            input.height(),
                            &kernel,
                            scale,
                            offset,
                        )
                    } else {
                        None
                    }
                    .unwrap_or_else(|| {
                        execute_filter3x3_native_bytes(
                            input.as_raw(),
                            input.width(),
                            input.height(),
                            $channels,
                            &kernel,
                            scale,
                            offset,
                            $uniform_shortcut,
                        )
                    });
                    $crate::raster::$carrier::from_raw(input.width(), input.height(), output)
                        .map($crate::raster::DynamicImage::$variant)
                        .ok_or_else(|| {
                            $crate::error::PilError::ValueError(
                                "filter output buffer does not match its mode".into(),
                            )
                        })
                }
                $crate::compute::pool_cpu::ops::filter::ops::FilterRequest::Filter5x5 {
                    kernel,
                    scale,
                    offset,
                } => {
                    let $crate::raster::DynamicImage::$variant(input) = image else {
                        return Err($crate::error::PilError::InternalError(
                            "filter mode strategy received a mismatched raster variant".into(),
                        ));
                    };
                    let output = ($filter5x5_impl)(
                        input.as_raw(),
                        input.width() as usize,
                        input.height() as usize,
                        $channels,
                        &kernel,
                        scale,
                        offset,
                    );
                    $crate::raster::$carrier::from_raw(input.width(), input.height(), output)
                        .map($crate::raster::DynamicImage::$variant)
                        .ok_or_else(|| {
                            $crate::error::PilError::ValueError(
                                "filter output buffer does not match its mode".into(),
                            )
                        })
                }
                request => {
                    let $crate::raster::DynamicImage::$variant(input) = image else {
                        return Err($crate::error::PilError::InternalError(
                            "filter mode strategy received a mismatched raster variant".into(),
                        ));
                    };
                    let output = execute_native_bytes_filter(
                        input.as_raw(),
                        input.width() as usize,
                        input.height() as usize,
                        $channels,
                        request,
                    )?;
                    let Some(output) = output else {
                        return Ok(image.clone());
                    };
                    $crate::raster::$carrier::from_raw(input.width(), input.height(), output)
                        .map($crate::raster::DynamicImage::$variant)
                        .ok_or_else(|| {
                            $crate::error::PilError::ValueError(
                                "filter output buffer does not match its mode".into(),
                            )
                        })
                }
            }
        }
    };
}

pub(crate) use native_filter_mode_leaf;

macro_rules! native_filter3x3_architectures {
    ($mode:expr, $channels:expr) => {
        const FILTER3X3_ARCHITECTURES: &[$crate::compute::registry::support::ImplementationSupport] = &[
            $crate::compute::registry::support::ImplementationSupport::cpu(
                $crate::compute::registry::support::OperationId::Filter3x3,
                $mode,
                $crate::compute::Architecture::Avx2Fma,
                cfg!(all(
                    any(target_arch = "x86", target_arch = "x86_64"),
                    target_feature = "avx2",
                    target_feature = "fma"
                )),
                &[$crate::compute::Architecture::Avx2Fma],
                $crate::compute::registry::support::StrategyGuard::Filter3x3NativeBytesVectorizable {
                    channels: $channels,
                },
            ),
            $crate::compute::registry::support::ImplementationSupport::cpu(
                $crate::compute::registry::support::OperationId::Filter3x3,
                $mode,
                $crate::compute::Architecture::Avx2,
                cfg!(all(
                    any(target_arch = "x86", target_arch = "x86_64"),
                    target_feature = "avx2",
                    not(target_feature = "fma")
                )),
                &[$crate::compute::Architecture::Avx2],
                $crate::compute::registry::support::StrategyGuard::Filter3x3NativeBytesVectorizable {
                    channels: $channels,
                },
            ),
            $crate::compute::registry::support::ImplementationSupport::cpu(
                $crate::compute::registry::support::OperationId::Filter3x3,
                $mode,
                $crate::compute::Architecture::AvxFma,
                cfg!(all(
                    any(target_arch = "x86", target_arch = "x86_64"),
                    target_feature = "avx",
                    target_feature = "fma",
                    not(target_feature = "avx2")
                )),
                &[$crate::compute::Architecture::AvxFma],
                $crate::compute::registry::support::StrategyGuard::Filter3x3NativeBytesVectorizable {
                    channels: $channels,
                },
            ),
            $crate::compute::registry::support::ImplementationSupport::cpu(
                $crate::compute::registry::support::OperationId::Filter3x3,
                $mode,
                $crate::compute::Architecture::Avx,
                cfg!(all(
                    any(target_arch = "x86", target_arch = "x86_64"),
                    target_feature = "avx",
                    not(target_feature = "avx2"),
                    not(target_feature = "fma")
                )),
                &[$crate::compute::Architecture::Avx],
                $crate::compute::registry::support::StrategyGuard::Filter3x3NativeBytesVectorizable {
                    channels: $channels,
                },
            ),
            $crate::compute::registry::support::ImplementationSupport::cpu(
                $crate::compute::registry::support::OperationId::Filter3x3,
                $mode,
                $crate::compute::Architecture::Sse2,
                cfg!(all(
                    any(target_arch = "x86", target_arch = "x86_64"),
                    target_feature = "sse2",
                    not(target_feature = "avx")
                )),
                &[$crate::compute::Architecture::Sse2],
                $crate::compute::registry::support::StrategyGuard::Filter3x3NativeBytesVectorizable {
                    channels: $channels,
                },
            ),
            $crate::compute::registry::support::ImplementationSupport::cpu(
                $crate::compute::registry::support::OperationId::Filter3x3,
                $mode,
                $crate::compute::Architecture::Neon,
                cfg!(all(target_arch = "aarch64", target_feature = "neon")),
                &[$crate::compute::Architecture::Neon],
                $crate::compute::registry::support::StrategyGuard::Filter3x3NativeBytesVectorizable {
                    channels: $channels,
                },
            ),
            $crate::compute::registry::support::ImplementationSupport::cpu(
                $crate::compute::registry::support::OperationId::Filter3x3,
                $mode,
                $crate::compute::Architecture::WasmSimd128,
                cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
                &[$crate::compute::Architecture::WasmSimd128],
                $crate::compute::registry::support::StrategyGuard::Filter3x3NativeBytesVectorizable {
                    channels: $channels,
                },
            ),
        ];
    };
}

pub(crate) use native_filter3x3_architectures;

macro_rules! native_filter5x5_architectures {
    ($mode:expr, $channels:expr) => {
        const FILTER5X5_ARCHITECTURES: &[$crate::compute::registry::support::ImplementationSupport] = &[
            $crate::compute::registry::support::ImplementationSupport::cpu(
                $crate::compute::registry::support::OperationId::Filter5x5,
                $mode,
                $crate::compute::Architecture::Avx2Fma,
                cfg!(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "avx2", target_feature = "fma")),
                &[$crate::compute::Architecture::Avx2Fma],
                $crate::compute::registry::support::StrategyGuard::Filter5x5NativeBytesVectorizable { channels: $channels },
            ),
            $crate::compute::registry::support::ImplementationSupport::cpu(
                $crate::compute::registry::support::OperationId::Filter5x5,
                $mode,
                $crate::compute::Architecture::Avx2,
                cfg!(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "avx2", not(target_feature = "fma"))),
                &[$crate::compute::Architecture::Avx2],
                $crate::compute::registry::support::StrategyGuard::Filter5x5NativeBytesVectorizable { channels: $channels },
            ),
            $crate::compute::registry::support::ImplementationSupport::cpu(
                $crate::compute::registry::support::OperationId::Filter5x5,
                $mode,
                $crate::compute::Architecture::AvxFma,
                cfg!(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "avx", target_feature = "fma", not(target_feature = "avx2"))),
                &[$crate::compute::Architecture::AvxFma],
                $crate::compute::registry::support::StrategyGuard::Filter5x5NativeBytesVectorizable { channels: $channels },
            ),
            $crate::compute::registry::support::ImplementationSupport::cpu(
                $crate::compute::registry::support::OperationId::Filter5x5,
                $mode,
                $crate::compute::Architecture::Avx,
                cfg!(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "avx", not(target_feature = "avx2"), not(target_feature = "fma"))),
                &[$crate::compute::Architecture::Avx],
                $crate::compute::registry::support::StrategyGuard::Filter5x5NativeBytesVectorizable { channels: $channels },
            ),
            $crate::compute::registry::support::ImplementationSupport::cpu(
                $crate::compute::registry::support::OperationId::Filter5x5,
                $mode,
                $crate::compute::Architecture::Sse2,
                cfg!(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2", not(target_feature = "avx"))),
                &[$crate::compute::Architecture::Sse2],
                $crate::compute::registry::support::StrategyGuard::Filter5x5NativeBytesVectorizable { channels: $channels },
            ),
            $crate::compute::registry::support::ImplementationSupport::cpu(
                $crate::compute::registry::support::OperationId::Filter5x5,
                $mode,
                $crate::compute::Architecture::Neon,
                cfg!(all(target_arch = "aarch64", target_feature = "neon")),
                &[$crate::compute::Architecture::Neon],
                $crate::compute::registry::support::StrategyGuard::Filter5x5NativeBytesVectorizable { channels: $channels },
            ),
            $crate::compute::registry::support::ImplementationSupport::cpu(
                $crate::compute::registry::support::OperationId::Filter5x5,
                $mode,
                $crate::compute::Architecture::WasmSimd128,
                cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
                &[$crate::compute::Architecture::WasmSimd128],
                $crate::compute::registry::support::StrategyGuard::Filter5x5NativeBytesVectorizable { channels: $channels },
            ),
        ];
    };
}

pub(crate) use native_filter5x5_architectures;

macro_rules! native_filter5x5_mode_fast_path {
    (none, $raw:expr, $width:expr, $height:expr, $kernel:expr, $scale:expr, $offset:expr) => {
        None::<Vec<u8>>
    };
    (rgb, $raw:expr, $width:expr, $height:expr, $kernel:expr, $scale:expr, $offset:expr) => {
        $crate::compute::pool_cpu::ops::filter::helpers::execute_filter5x5_rgb_vector_fast_path(
            $raw, $width, $height, $kernel, $scale, $offset,
        )
    };
    (la, $raw:expr, $width:expr, $height:expr, $kernel:expr, $scale:expr, $offset:expr) => {
        $crate::compute::pool_cpu::ops::filter::luma_a8::helpers::filter5x5::execute_special(
            $raw, $width, $height, $kernel, $scale, $offset,
        )
    };
    (luma, $raw:expr, $width:expr, $height:expr, $kernel:expr, $scale:expr, $offset:expr) => {
        $crate::compute::pool_cpu::ops::filter::luma8::helpers::execute_special(
            $raw, $width, $height, $kernel, $scale, $offset,
        )
    };
}

pub(crate) use native_filter5x5_mode_fast_path;

macro_rules! native_filter5x5_arch_leaf {
    ($variant:ident, $carrier:ident, $channels:expr) => {
        $crate::compute::pool_cpu::ops::filter::helpers::native_filter5x5_arch_leaf!(
            @impl, $variant, $carrier, $channels, none
        );
    };
    ($variant:ident, $carrier:ident, $channels:expr, $special:ident) => {
        $crate::compute::pool_cpu::ops::filter::helpers::native_filter5x5_arch_leaf!(
            @impl, $variant, $carrier, $channels, $special
        );
    };
    (@impl, $variant:ident, $carrier:ident, $channels:expr, $special:ident) => {
        pub(super) fn execute_filter5x5_small_uniform(
            image: &crate::raster::DynamicImage,
            request: $crate::compute::pool_cpu::ops::filter::ops::FilterRequest,
        ) -> Result<Option<crate::raster::DynamicImage>, $crate::error::PilError> {
            let $crate::compute::pool_cpu::ops::filter::ops::FilterRequest::Filter5x5 {
                kernel,
                scale,
                offset,
            } = request
            else {
                return Err($crate::error::PilError::InternalError(
                    "small-uniform Filter5x5 strategy received another filter request".into(),
                ));
            };
            let $crate::raster::DynamicImage::$variant(input) = image else {
                return Err($crate::error::PilError::InternalError(
                    "small-uniform Filter5x5 strategy received a mismatched raster variant".into(),
                ));
            };
            let Some(output) =
                $crate::compute::pool_cpu::ops::filter::helpers::execute_filter5x5_small_uniform_bytes(
                    input.as_raw(),
                    input.width(),
                    input.height(),
                    $channels,
                    &kernel,
                    scale,
                    offset,
                )
            else {
                return Ok(None);
            };
            $crate::raster::$carrier::from_raw(input.width(), input.height(), output)
                .map($crate::raster::DynamicImage::$variant)
                .map(Some)
                .ok_or_else(|| {
                    $crate::error::PilError::ValueError(
                        "filter output buffer does not match its mode".into(),
                    )
                })
        }

        pub(super) fn execute_filter5x5_vectorized(
            image: &crate::raster::DynamicImage,
            request: $crate::compute::pool_cpu::ops::filter::ops::FilterRequest,
        ) -> Result<crate::raster::DynamicImage, $crate::error::PilError> {
            let $crate::compute::pool_cpu::ops::filter::ops::FilterRequest::Filter5x5 {
                kernel,
                scale,
                offset,
            } = request
            else {
                return Err($crate::error::PilError::InternalError(
                    "vector Filter5x5 strategy received another filter request".into(),
                ));
            };
            let $crate::raster::DynamicImage::$variant(input) = image else {
                return Err($crate::error::PilError::InternalError(
                    "vector Filter5x5 strategy received a mismatched raster variant".into(),
                ));
            };
            if input.width() < 5 || input.height() < 5 {
                let output = $crate::compute::pool_cpu::ops::filter::helpers::execute_native_filter_identity_copy(
                    input.as_raw(),
                )?;
                return $crate::raster::$carrier::from_raw(input.width(), input.height(), output)
                    .map($crate::raster::DynamicImage::$variant)
                    .ok_or_else(|| {
                        $crate::error::PilError::InternalError(
                            "native Filter5x5 identity output does not match its mode".into(),
                        )
                    });
            }
            let output = if let Some(output) =
                $crate::compute::pool_cpu::ops::filter::helpers::native_filter5x5_mode_fast_path!(
                    $special,
                    input.as_raw(),
                    input.width(),
                    input.height(),
                    &kernel,
                    scale,
                    offset
                )
            {
                output
            } else {
                let normalized = std::array::from_fn(|index| kernel[index] / scale);
                let mut output = input.as_raw().clone();
                let (vector_blocks, scalar_tail) =
                    $crate::compute::pool_cpu::ops::filter::helpers::apply_filter5x5_vector(
                        input.as_raw(),
                        &mut output,
                        input.width() as usize,
                        input.height() as usize,
                        $channels,
                        &normalized,
                        offset + 0.5,
                    )
                    .ok_or_else(|| {
                        $crate::error::PilError::ValueError(
                            "filter output buffer does not match its mode".into(),
                        )
                    })?;
                $crate::compute::record_pipeline_operation_path("vector");
                if vector_blocks != 0 {
                    $crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
                }
                if scalar_tail != 0 {
                    $crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
                }
                output
            };
            $crate::raster::$carrier::from_raw(input.width(), input.height(), output)
                .map($crate::raster::DynamicImage::$variant)
                .ok_or_else(|| {
                    $crate::error::PilError::ValueError(
                        "filter output buffer does not match its mode".into(),
                    )
                })
        }
    };
}

pub(crate) use native_filter5x5_arch_leaf;

macro_rules! native_filter5x5_arch_dispatch {
    ($architecture:expr, $image:expr, $request:expr, $default:path, $vectorized:path) => {{
        match $architecture {
            $crate::compute::Architecture::Default => $default($image, $request),
            $crate::compute::Architecture::Sse2
            | $crate::compute::Architecture::Avx
            | $crate::compute::Architecture::AvxFma
            | $crate::compute::Architecture::Avx2
            | $crate::compute::Architecture::Avx2Fma
            | $crate::compute::Architecture::Neon
            | $crate::compute::Architecture::WasmSimd128 => $vectorized($image, $request),
            _ => Err($crate::error::PilError::InternalError(
                "filter selected an unowned Filter5x5 architecture".into(),
            )),
        }
    }};
}

pub(crate) use native_filter5x5_arch_dispatch;

macro_rules! native_filter3x3_arch_leaf {
    ($variant:ident, $carrier:ident, $channels:expr) => {
        $crate::compute::pool_cpu::ops::filter::helpers::native_filter3x3_arch_leaf!(
            @impl, $variant, $carrier, $channels, false
        );
    };
    ($variant:ident, $carrier:ident, $channels:expr, $la_binomial:expr) => {
        $crate::compute::pool_cpu::ops::filter::helpers::native_filter3x3_arch_leaf!(
            @impl, $variant, $carrier, $channels, $la_binomial
        );
    };
    (@impl, $variant:ident, $carrier:ident, $channels:expr, $la_binomial:expr) => {
        pub(super) fn execute_filter3x3_vectorized(
            image: &crate::raster::DynamicImage,
            request: $crate::compute::pool_cpu::ops::filter::ops::FilterRequest,
        ) -> Result<crate::raster::DynamicImage, $crate::error::PilError> {
            let $crate::compute::pool_cpu::ops::filter::ops::FilterRequest::Filter3x3 {
                kernel,
                scale,
                offset,
            } = request
            else {
                return Err($crate::error::PilError::InternalError(
                    "vector Filter3x3 strategy received another filter request".into(),
                ));
            };
            let $crate::raster::DynamicImage::$variant(input) = image else {
                return Err($crate::error::PilError::InternalError(
                    "vector Filter3x3 strategy received a mismatched raster variant".into(),
                ));
            };
            if input.width() < 3 || input.height() < 3 {
                let output = $crate::compute::pool_cpu::ops::filter::helpers::execute_native_filter_identity_copy(
                    input.as_raw(),
                )?;
                return $crate::raster::$carrier::from_raw(input.width(), input.height(), output)
                    .map($crate::raster::DynamicImage::$variant)
                    .ok_or_else(|| {
                        $crate::error::PilError::InternalError(
                            "native Filter3x3 identity output does not match its mode".into(),
                        )
                    });
            }
            let la_binomial = if $la_binomial {
                $crate::compute::pool_cpu::ops::filter::helpers::execute_filter3x3_la_binomial_vector_bytes(
                    input.as_raw(),
                    input.width(),
                    input.height(),
                    &kernel,
                    scale,
                    offset,
                )
            } else {
                None
            };
            let output = if let Some((output, vector_blocks, scalar_tail)) = la_binomial {
                $crate::compute::record_pipeline_operation_path("vector");
                if vector_blocks != 0 {
                    $crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
                }
                if scalar_tail != 0 {
                    $crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
                }
                output
            } else if let Some(output) =
                $crate::compute::pool_cpu::ops::filter::helpers::execute_filter3x3_small_uniform_bytes(
                    input.as_raw(),
                    input.width(),
                    input.height(),
                    $channels,
                    &kernel,
                    scale,
                    offset,
                )
            {
                let input_bytes = input.as_raw();
                if input_bytes[..$channels] == output[..$channels] {
                    $crate::compute::pool_cpu::ops::filter::helpers::execute_native_filter_identity_copy(
                        input_bytes,
                    )?
                } else {
                    $crate::compute::record_pipeline_operation_path("scalar-control");
                    output
                }
            } else {
                let normalized = std::array::from_fn(|index| kernel[index] / scale);
                let mut output = input.as_raw().clone();
                let (vector_blocks, scalar_tail) =
                    $crate::compute::pool_cpu::ops::filter::helpers::apply_filter3x3_vector(
                        input.as_raw(),
                        &mut output,
                        input.width() as usize,
                        input.height() as usize,
                        $channels,
                        &normalized,
                        offset + 0.5,
                    )
                    .ok_or_else(|| {
                        $crate::error::PilError::ValueError(
                            "filter output buffer does not match its mode".into(),
                        )
                    })?;
                $crate::compute::record_pipeline_operation_path("vector");
                if vector_blocks != 0 {
                    $crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
                }
                if scalar_tail != 0 {
                    $crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
                }
                output
            };
            $crate::raster::$carrier::from_raw(input.width(), input.height(), output)
                .map($crate::raster::DynamicImage::$variant)
                .ok_or_else(|| {
                    $crate::error::PilError::ValueError(
                        "filter output buffer does not match its mode".into(),
                    )
                })
        }
    };
}

pub(crate) use native_filter3x3_arch_leaf;
