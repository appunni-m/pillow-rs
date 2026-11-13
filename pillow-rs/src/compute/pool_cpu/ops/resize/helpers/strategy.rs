//! Selected implementation checks for native Resize mode owners.

use crate::compute::registry::support::{OperationId, StrategySelection};
use crate::compute::{Architecture, Backend, ExecutionContext};
use crate::error::PilError;
use crate::pipeline::ResampleFilter;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn require_default_architecture(
    context: &ExecutionContext,
    selection: &StrategySelection,
) -> Result<(), PilError> {
    let implementation = selection.implementation;
    if context.backend() != Backend::Cpu || implementation.backend != Backend::Cpu {
        return Err(PilError::InternalError(
            "CPU Resize mode received a non-CPU execution context".into(),
        ));
    }
    if !matches!(
        implementation.operation,
        OperationId::Resize | OperationId::ResizeBoxed
    ) || selection.requested_architecture != context.architecture()
    {
        return Err(PilError::InternalError(
            "Resize mode received a mismatched strategy selection".into(),
        ));
    }
    if implementation.architecture != Architecture::Default {
        return Err(PilError::InternalError(
            "Resize mode selected an architecture without a declared strategy".into(),
        ));
    }
    Ok(())
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn require_architecture(
    context: &ExecutionContext,
    selection: &StrategySelection,
) -> Result<Architecture, PilError> {
    let implementation = selection.implementation;
    let architecture = implementation.architecture;
    if context.backend() != Backend::Cpu || implementation.backend != Backend::Cpu {
        return Err(PilError::InternalError(
            "CPU Resize mode received a non-CPU execution context".into(),
        ));
    }
    if !matches!(
        implementation.operation,
        OperationId::Resize | OperationId::ResizeBoxed
    ) || selection.requested_architecture != context.architecture()
    {
        return Err(PilError::InternalError(
            "Resize mode received a mismatched strategy selection".into(),
        ));
    }
    if architecture == Architecture::Default
        || !context.permits_architecture(architecture)
        || !context.supports_architecture(architecture)
    {
        return Err(PilError::InternalError(
            "Resize mode received an unavailable architecture strategy".into(),
        ));
    }
    Ok(architecture)
}

/// Selects a declared native-byte nearest kernel or the mode's Default route.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn resize_byte_nearest<T, const CHANNELS: usize>(
    context: &ExecutionContext,
    selection: &StrategySelection,
    source: &[u8],
    source_size: (u32, u32),
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: super::typed_nearest::BoxBounds,
    default: impl FnOnce() -> Result<T, PilError>,
    from_bytes: impl FnOnce(Vec<u8>) -> Result<T, PilError>,
) -> Result<T, PilError> {
    let architecture = selection.implementation.architecture;
    if architecture == Architecture::Default {
        require_default_architecture(context, selection)?;
        return default();
    }
    if !matches!(filter, ResampleFilter::Nearest) {
        return Err(PilError::InternalError(
            "native-byte nearest Resize received an unsupported request".into(),
        ));
    }
    require_architecture(context, selection)?;
    match architecture {
        #[cfg(all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "sse2"
        ))]
        Architecture::Sse2 => (),
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        Architecture::Neon => (),
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        Architecture::WasmSimd128 => (),
        _ => {
            return Err(PilError::InternalError(format!(
                "native-byte Resize received unsupported architecture {architecture:?}"
            )));
        }
    }
    let output = match bounds {
        Some(bounds) => super::native_nearest::execute_boxed::<CHANNELS>(
            source,
            source_size,
            (width, height),
            bounds,
        )?,
        None => super::native_nearest::execute::<CHANNELS>(source, source_size, (width, height))?,
    };
    from_bytes(output)
}

/// Selects the mode's serial Default route or its declared native-byte strategy.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn resize_byte_native<T, const CHANNELS: usize>(
    context: &ExecutionContext,
    selection: &StrategySelection,
    source: &[u8],
    source_size: (u32, u32),
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: super::typed_nearest::BoxBounds,
    default: impl FnOnce() -> Result<T, PilError>,
    filtered_vector: impl FnOnce(Option<(f64, f64, f64, f64)>) -> Result<Vec<u8>, PilError>,
    from_bytes: impl FnOnce(Vec<u8>) -> Result<T, PilError>,
) -> Result<T, PilError> {
    let architecture = selection.implementation.architecture;
    if architecture == Architecture::Default {
        require_default_architecture(context, selection)?;
        return default();
    }
    let _architecture = require_architecture(context, selection)?;
    let output = match (filter, bounds) {
        (ResampleFilter::Nearest, Some(bounds)) => {
            super::native_nearest::execute_boxed::<CHANNELS>(
                source,
                source_size,
                (width, height),
                bounds,
            )?
        }
        (ResampleFilter::Nearest, None) => {
            super::native_nearest::execute::<CHANNELS>(source, source_size, (width, height))?
        }
        (_, bounds) => filtered_vector(bounds)?,
    };
    from_bytes(output)
}
