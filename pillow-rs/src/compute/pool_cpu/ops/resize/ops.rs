//! CPU Resize mode support and strategy orchestration.

use crate::compute::{Architecture, Backend, ExecutionContext};
use crate::error::PilError;
use crate::image::Mode;
use crate::ops::pil_resize::{premultiply_alpha, unpremultiply_alpha};
use crate::pipeline::ResampleFilter;
use crate::raster::DynamicImage;

use super::{f, i, kernels, one};

pub(crate) use crate::ops::pil_resize::resample_kernel;
pub(crate) use crate::ops::pil_resize::{F64MulAdd, f_resize_samples_allow_finite_fma};
#[cfg(target_arch = "x86_64")]
pub(crate) use crate::ops::pil_resize::X86FmaToken;
pub(crate) use f::ops::{resize_f, resize_f_in_context, resize_f_with_fma};
pub(crate) use i::ops::{
    i32_sample_bytes_from_native_storage, i32_samples_from_native_storage,
    resize_i_boxed_in_context, resize_i_contiguous_eight_tap_weights, resize_i_in_context,
    resize_i_round_up_to_i32, resize_i_sum_eight, resize_i_with_fma,
};

#[derive(Clone, Copy)]
enum ResizeStorage {
    L8,
    La8,
    Rgb8,
    Rgba8,
    L16,
    La16,
    Rgb16,
    Rgba16,
    Rgb32F,
    Rgba32F,
}

#[derive(Clone, Copy)]
enum ResizeStrategy {
    Native,
    One,
    I32,
    F32,
}

#[derive(Clone, Copy)]
enum ResizeArchitectureSupport {
    DefaultOnly,
    FmaWithScalarFallback,
}

/// The family contract for one logical mode, including its native carrier and
/// mode-preserving output identity. Architecture-specific kernels are eligible
/// only within the selected mode strategy.
struct ResizeModeSupport {
    input_storage: ResizeStorage,
    output_mode: Mode,
    strategy: ResizeStrategy,
    architecture: ResizeArchitectureSupport,
}

const fn architecture_support(strategy: ResizeStrategy) -> ResizeArchitectureSupport {
    match strategy {
        ResizeStrategy::I32 | ResizeStrategy::F32 => {
            ResizeArchitectureSupport::FmaWithScalarFallback
        }
        ResizeStrategy::Native | ResizeStrategy::One => ResizeArchitectureSupport::DefaultOnly,
    }
}

/// Declare Resize support for every Pillow and internal sample mode.
const fn mode_support(mode: Mode) -> ResizeModeSupport {
    use Mode as M;
    use ResizeStorage as S;
    use ResizeStrategy as R;

    let (input_storage, strategy) = match mode {
        M::One => (S::L8, R::One),
        M::ImageLuma8 | M::ImagePalette8 => (S::L8, R::Native),
        M::ImageLumaA8 | M::ImageLaPremultiplied8 | M::ImagePaletteAlpha8 => {
            (S::La8, R::Native)
        }
        M::ImageRgb8 | M::ImageYCbCr8 | M::ImageLab8 | M::ImageHsv8 => (S::Rgb8, R::Native),
        M::ImageRgba8
        | M::ImageRgbPremultiplied8
        | M::ImageRgbx8
        | M::ImageCmyk8 => (S::Rgba8, R::Native),
        M::ImageI32 => (S::Rgba8, R::I32),
        M::ImageF32 => (S::Rgba8, R::F32),
        M::ImageI16 | M::ImageI16L | M::ImageI16B | M::ImageI16N => (S::L16, R::Native),
        // NO_PILLOW_SUPPORT: internal LA16 storage identity.
        M::ImageLumaA16 => (S::La16, R::Native),
        // NO_PILLOW_SUPPORT: internal RGB16 storage identity.
        M::ImageRgb16 => (S::Rgb16, R::Native),
        // NO_PILLOW_SUPPORT: internal RGBA16 storage identity.
        M::ImageRgba16 => (S::Rgba16, R::Native),
        // NO_PILLOW_SUPPORT: internal RGB32F storage identity.
        M::ImageRgb32F => (S::Rgb32F, R::Native),
        // NO_PILLOW_SUPPORT: internal RGBA32F storage identity.
        M::ImageRgba32F => (S::Rgba32F, R::Native),
    };
    ResizeModeSupport {
        input_storage,
        output_mode: mode,
        architecture: architecture_support(strategy),
        strategy,
    }
}

fn storage_matches(image: &DynamicImage, storage: ResizeStorage) -> bool {
    matches!(
        (storage, image),
        (ResizeStorage::L8, DynamicImage::ImageLuma8(_))
            | (ResizeStorage::La8, DynamicImage::ImageLumaA8(_))
            | (ResizeStorage::Rgb8, DynamicImage::ImageRgb8(_))
            | (ResizeStorage::Rgba8, DynamicImage::ImageRgba8(_))
            | (ResizeStorage::L16, DynamicImage::ImageLuma16(_))
            | (ResizeStorage::La16, DynamicImage::ImageLumaA16(_))
            | (ResizeStorage::Rgb16, DynamicImage::ImageRgb16(_))
            | (ResizeStorage::Rgba16, DynamicImage::ImageRgba16(_))
            | (ResizeStorage::Rgb32F, DynamicImage::ImageRgb32F(_))
            | (ResizeStorage::Rgba32F, DynamicImage::ImageRgba32F(_))
    )
}

fn validate_resize_context(
    context: &ExecutionContext,
    support: &ResizeModeSupport,
) -> Result<(), PilError> {
    let architecture_matches = match support.architecture {
        ResizeArchitectureSupport::DefaultOnly => context.architecture().supports_cpu(),
        ResizeArchitectureSupport::FmaWithScalarFallback => matches!(
            context.architecture(),
            Architecture::Scalar | Architecture::Fma
        ),
    };
    if matches!(context.backend(), Backend::Cpu | Backend::ParallelCpu) && architecture_matches {
        return Ok(());
    }
    Err(PilError::InternalError(format!(
        "CPU Resize received {:?}/{:?} context",
        context.backend(),
        context.architecture()
    )))
}

fn validate_mode_storage(image: &DynamicImage, mode: Mode, support: &ResizeModeSupport) -> Result<(), PilError> {
    if storage_matches(image, support.input_storage) {
        return Ok(());
    }
    Err(PilError::InternalError(format!(
        "Resize mode {mode:?} does not match its native storage"
    )))
}

fn compatibility_mode(image: &DynamicImage, name: Option<&str>) -> Result<Mode, PilError> {
    match name {
        Some(name) => Mode::from_pillow_name(name)
            .ok_or_else(|| PilError::ValueError(format!("unsupported Pillow mode: {name}"))),
        None => Ok(Mode::for_image(image, None)),
    }
}

/// TODO(pillow-rs#3ptb): Remove this compatibility adapter after direct
/// registry callers receive the wrapper's cached `Mode` selector.
///
/// Direct callers that still provide a Pillow spelling are translated once
/// here. Pipeline routing passes its cached `Mode` to the typed context entry.
pub(crate) fn execute_resize(
    image: &DynamicImage,
    width: u32,
    height: u32,
    filter: &ResampleFilter,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let mode = compatibility_mode(image, mode)?;
    execute_resize_in_context(
        &ExecutionContext::for_backend(Backend::Cpu),
        image,
        width,
        height,
        filter,
        mode,
    )
}

/// Execute Resize with the wrapper's precomputed mode and injected CPU target.
pub(crate) fn execute_resize_in_context(
    context: &ExecutionContext,
    image: &DynamicImage,
    width: u32,
    height: u32,
    filter: &ResampleFilter,
    mode: Mode,
) -> Result<DynamicImage, PilError> {
    let support = mode_support(mode);
    validate_resize_context(context, &support)?;
    validate_mode_storage(image, mode, &support)?;
    debug_assert_eq!(support.output_mode, mode, "Resize preserves its mode");

    match support.strategy {
        ResizeStrategy::One => Ok(one::ops::resize(image, width, height, filter)),
        ResizeStrategy::I32 => i::ops::resize(context, image, width, height, filter),
        ResizeStrategy::F32 => f::ops::resize(context, image, width, height, filter),
        ResizeStrategy::Native => Ok(kernels::default::resize(
            image, width, height, filter, mode,
        )),
    }
}

/// Compatibility adapter for direct boxed-resize callers.
pub(crate) fn execute_resize_boxed(
    image: &DynamicImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: (f64, f64, f64, f64),
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let mode = compatibility_mode(image, mode)?;
    execute_resize_boxed_in_context(
        &ExecutionContext::for_backend(Backend::Cpu),
        image,
        width,
        height,
        filter,
        bounds,
        mode,
    )
}

/// Execute boxed Resize with the wrapper's precomputed mode and CPU target.
pub(crate) fn execute_resize_boxed_in_context(
    context: &ExecutionContext,
    image: &DynamicImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: (f64, f64, f64, f64),
    mode: Mode,
) -> Result<DynamicImage, PilError> {
    let support = mode_support(mode);
    validate_resize_context(context, &support)?;
    validate_mode_storage(image, mode, &support)?;
    debug_assert_eq!(support.output_mode, mode, "Resize preserves its mode");

    if u64::from(image.height()) <= u64::from(image.width()) * 100 || height >= image.height() {
        return resize_boxed_once(context, image, width, height, bounds, filter, mode);
    }

    let alpha_mode = premultiplied_mode(image, mode, filter);
    let source = alpha_mode.map(|_| premultiply_alpha(image));
    let working_mode = alpha_mode.unwrap_or(mode);
    let vertical = resize_boxed_once(
        context,
        source.as_ref().unwrap_or(image),
        image.width(),
        height,
        (0.0, bounds.1, f64::from(image.width()), bounds.3),
        filter,
        working_mode,
    )?;
    let result = resize_boxed_once(
        context,
        &vertical,
        width,
        height,
        (bounds.0, 0.0, bounds.2, f64::from(height)),
        filter,
        working_mode,
    )?;
    Ok(if alpha_mode.is_some() {
        unpremultiply_alpha(&result)
    } else {
        result
    })
}

fn premultiplied_mode(
    image: &DynamicImage,
    mode: Mode,
    filter: ResampleFilter,
) -> Option<Mode> {
    if matches!(filter, ResampleFilter::Nearest) {
        return None;
    }
    match (image, mode) {
        (DynamicImage::ImageRgba8(_), Mode::ImageRgba8) => Some(Mode::ImageRgbPremultiplied8),
        (DynamicImage::ImageLumaA8(_), Mode::ImageLumaA8) => Some(Mode::ImageLaPremultiplied8),
        _ => None,
    }
}

fn resize_boxed_once(
    context: &ExecutionContext,
    image: &DynamicImage,
    width: u32,
    height: u32,
    bounds: (f64, f64, f64, f64),
    filter: ResampleFilter,
    mode: Mode,
) -> Result<DynamicImage, PilError> {
    let strategy = mode_support(mode).strategy;
    match strategy {
        ResizeStrategy::I32 if !matches!(filter, ResampleFilter::Nearest) => {
            i::ops::resize_boxed(context, image, width, height, bounds, &filter)
        }
        ResizeStrategy::F32 if matches!(image, DynamicImage::ImageRgba8(_)) => {
            Ok(crate::ops::pil_resize::pil_resize_boxed_in_context(
                context,
                image,
                width,
                height,
                bounds.0,
                bounds.1,
                bounds.2,
                bounds.3,
                filter,
                mode.pillow_name(),
            ))
        }
        _ if matches!(filter, ResampleFilter::Nearest) && !matches!(mode, Mode::ImageF32) => {
            Ok(kernels::default::resize_boxed_raw(
                image, width, height, bounds, filter,
            ))
        }
        _ => Ok(kernels::default::resize_boxed(
            image, width, height, bounds, filter, mode,
        )),
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
