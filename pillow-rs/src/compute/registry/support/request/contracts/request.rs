//! Operation-parameter contract assessment.

use super::{
    InputContract, RequestFacts, StrategyInputs, UnresolvedGuard, unresolved_request, wrong_request,
};
use crate::error::PilError;
use crate::raster::Mode;

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(super) fn assess(
    contract: InputContract,
    input: StrategyInputs,
) -> Result<Option<UnresolvedGuard>, PilError> {
    if input.request == RequestFacts::Unspecified {
        return match contract {
            InputContract::Filter3x3Parameters
            | InputContract::Filter5x5Parameters
            | InputContract::GaussianBlurSigma
            | InputContract::BoxBlurParameters
            | InputContract::BoxBlurXYParameters
            | InputContract::UnsharpMaskParameters
            | InputContract::SharpnessFactor
            | InputContract::EffectSpreadDistance
            | InputContract::EffectNoiseSigma
            | InputContract::EffectMandelbrotParameters
            | InputContract::QuantizeParameters { .. }
            | InputContract::Rotate
            | InputContract::TransformParameters
            | InputContract::PosterizeBits
            | InputContract::ByteLutEntries { .. }
            | InputContract::PointI32LutEntries { .. }
            | InputContract::PointFloatLutEntries { .. }
            | InputContract::ReduceFactors { .. }
            | InputContract::Thumbnail
            | InputContract::ContainParameters
            | InputContract::CoverParameters
            | InputContract::FitParameters
            | InputContract::PadParameters
            | InputContract::ScaleParameters
            | InputContract::OddRankWindow { .. }
            | InputContract::ModeFilterWindow => unresolved_request(contract),
            _ => Err(PilError::InternalError(
                "request validation received a non-request input contract".into(),
            )),
        };
    }

    match contract {
        InputContract::EffectSpreadDistance | InputContract::EffectNoiseSigma => {
            assess_effect_request(contract, input.request)
        }
        InputContract::QuantizeParameters { .. } => assess_quantize_request(contract, input),
        InputContract::EffectMandelbrotParameters => assess_mandelbrot_request(input),
        InputContract::Rotate => match input.request {
            RequestFacts::Rotate { .. } => Ok(None),
            _ => Err(wrong_request("Rotate")),
        },
        InputContract::TransformParameters => assess_transform_request(input.request),
        InputContract::Filter3x3Parameters
        | InputContract::Filter5x5Parameters
        | InputContract::GaussianBlurSigma
        | InputContract::BoxBlurParameters
        | InputContract::BoxBlurXYParameters
        | InputContract::UnsharpMaskParameters
        | InputContract::SharpnessFactor => assess_filter_request(contract, input.request),
        InputContract::PosterizeBits => assess_posterize_request(input.request),
        InputContract::ByteLutEntries { .. } => assess_byte_lut_request(contract, input.request),
        InputContract::PointI32LutEntries { .. } => {
            assess_point_i32_lut_request(contract, input.request)
        }
        InputContract::PointFloatLutEntries { .. } => {
            assess_point_float_lut_request(contract, input.request)
        }
        InputContract::ReduceFactors { .. } => assess_reduce_request(contract, input.request),
        InputContract::Thumbnail => assess_thumbnail_request(input),
        InputContract::ContainParameters
        | InputContract::CoverParameters
        | InputContract::FitParameters
        | InputContract::PadParameters
        | InputContract::ScaleParameters => assess_imageops_request(contract, input.request),
        InputContract::OddRankWindow { .. } => assess_rank_window(contract, input.request),
        InputContract::ModeFilterWindow => assess_mode_filter_window(input.request),
        _ => Err(PilError::InternalError(
            "request validation received a non-request input contract".into(),
        )),
    }
}

fn assess_effect_request(
    contract: InputContract,
    request: RequestFacts,
) -> Result<Option<UnresolvedGuard>, PilError> {
    match (contract, request) {
        (InputContract::EffectSpreadDistance, RequestFacts::EffectSpread { .. })
        | (InputContract::EffectNoiseSigma, RequestFacts::EffectNoise { .. }) => Ok(None),
        (InputContract::EffectSpreadDistance, _) => Err(wrong_request("EffectSpread")),
        (InputContract::EffectNoiseSigma, _) => Err(wrong_request("EffectNoise")),
        _ => Err(PilError::InternalError(
            "effect request assessment received another contract".into(),
        )),
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pass@quantize-supplied-palette-20261112-cpu-default; benchmark=pending
fn assess_quantize_request(
    contract: InputContract,
    input: StrategyInputs,
) -> Result<Option<UnresolvedGuard>, PilError> {
    let InputContract::QuantizeParameters {
        min_colors,
        max_colors,
        recognized_methods,
        method_restricted_modes,
        allowed_methods,
        palette_mode: expected_palette_mode,
        empty_palette_rejection,
        palette_sources,
        palette_carriers,
        palette_carrier_rejection,
        unavailable_methods,
        kmeans_nonnegative,
        colors_rejection,
        invalid_method_rejection,
        pillow_rgba_method_rejection,
        alpha_method_rejection,
        palette_mode_rejection,
        palette_source_rejection,
        unavailable_method_rejection,
        kmeans_rejection,
    } = contract
    else {
        unreachable!("Quantize request assessment received another contract");
    };
    let RequestFacts::Quantize {
        source_mode,
        visible_mode,
        colors,
        method,
        kmeans,
        palette_mode,
        palette_entries,
    } = input.request
    else {
        return Err(wrong_request("Quantize"));
    };

    let admission_mode = visible_mode.unwrap_or(source_mode);
    if method_restricted_modes.contains(&admission_mode) && !allowed_methods.contains(&method) {
        let rejection = if admission_mode == Mode::ImageRgba8 {
            pillow_rgba_method_rejection
        } else {
            alpha_method_rejection
        };
        return Err(PilError::ValueError(rejection.into()));
    }
    if let Some(palette_mode) = palette_mode {
        if palette_mode != expected_palette_mode {
            return Err(PilError::ValueError(palette_mode_rejection.into()));
        }
        if !palette_sources.contains(&admission_mode) {
            return Err(PilError::ValueError(palette_source_rejection.into()));
        }
        if !palette_carriers.contains(&source_mode) {
            return Err(PilError::ValueError(palette_carrier_rejection.into()));
        }
        if palette_entries == Some(0) {
            return Err(PilError::ValueError(empty_palette_rejection.into()));
        }
        return Ok(None);
    }
    if kmeans_nonnegative && kmeans.is_some_and(|iterations| iterations < 0) {
        return Err(PilError::ValueError(kmeans_rejection.into()));
    }
    if input.width != 0 && input.height != 0 {
        if colors < min_colors || colors > max_colors {
            return Err(PilError::ValueError(colors_rejection.into()));
        }
        if !recognized_methods.contains(&method) {
            return Err(PilError::ValueError(invalid_method_rejection.into()));
        }
        if unavailable_methods.contains(&method) {
            return Err(PilError::ValueError(unavailable_method_rejection.into()));
        }
    }
    Ok(None)
}

fn assess_mandelbrot_request(input: StrategyInputs) -> Result<Option<UnresolvedGuard>, PilError> {
    let RequestFacts::EffectMandelbrot {
        extent_bits,
        quality,
    } = input.request
    else {
        return Err(wrong_request("EffectMandelbrot"));
    };
    if input.width == 0 || input.height == 0 {
        return Ok(None);
    }
    let (x0, y0, x1, y1) = extent_bits;
    let extent_width = f64::from_bits(x1) - f64::from_bits(x0);
    let extent_height = f64::from_bits(y1) - f64::from_bits(y0);
    if extent_width < 0.0 || extent_height < 0.0 || quality < 2 {
        Err(PilError::ValueError("unrecognized argument value".into()))
    } else {
        Ok(None)
    }
}

fn assess_transform_request(request: RequestFacts) -> Result<Option<UnresolvedGuard>, PilError> {
    let RequestFacts::Transform {
        method, data_len, ..
    } = request
    else {
        return Err(wrong_request("Transform"));
    };
    match method {
        0 if data_len != 6 => Err(PilError::ValueError(
            "wrong number of matrix entries".into(),
        )),
        1 | 2 if data_len != 8 => Err(PilError::ValueError("wrong number of data points".into())),
        0..=3 => Ok(None),
        _ => Err(wrong_request("Transform")),
    }
}

fn assess_filter_request(
    contract: InputContract,
    request: RequestFacts,
) -> Result<Option<UnresolvedGuard>, PilError> {
    match (contract, request) {
        (InputContract::Filter3x3Parameters, RequestFacts::Filter3x3 { .. })
        | (InputContract::Filter5x5Parameters, RequestFacts::Filter5x5 { .. })
        | (InputContract::GaussianBlurSigma, RequestFacts::GaussianBlur { .. })
        | (InputContract::BoxBlurParameters, RequestFacts::BoxBlur { .. })
        | (InputContract::BoxBlurXYParameters, RequestFacts::BoxBlurXY { .. })
        | (InputContract::UnsharpMaskParameters, RequestFacts::UnsharpMask { .. })
        | (InputContract::SharpnessFactor, RequestFacts::Sharpness { .. }) => Ok(None),
        (InputContract::Filter3x3Parameters, _) => Err(wrong_request("Filter3x3")),
        (InputContract::Filter5x5Parameters, _) => Err(wrong_request("Filter5x5")),
        (InputContract::GaussianBlurSigma, _) => Err(wrong_request("GaussianBlur")),
        (InputContract::BoxBlurParameters, _) => Err(wrong_request("BoxBlur")),
        (InputContract::BoxBlurXYParameters, _) => Err(wrong_request("BoxBlurXY")),
        (InputContract::UnsharpMaskParameters, _) => Err(wrong_request("UnsharpMask")),
        (InputContract::SharpnessFactor, _) => Err(wrong_request("Sharpness")),
        _ => Err(PilError::InternalError(
            "filter request assessment received another contract".into(),
        )),
    }
}

fn assess_posterize_request(request: RequestFacts) -> Result<Option<UnresolvedGuard>, PilError> {
    match request {
        RequestFacts::Posterize { bits } if (1..=8).contains(&bits) => Ok(None),
        _ => Err(PilError::ValueError(
            "posterize bits must be in range 1..=8".into(),
        )),
    }
}

fn assess_byte_lut_request(
    contract: InputContract,
    request: RequestFacts,
) -> Result<Option<UnresolvedGuard>, PilError> {
    let InputContract::ByteLutEntries { channels } = contract else {
        unreachable!("byte LUT request assessment received another contract");
    };
    match request {
        RequestFacts::EvalLut { entries } if entries == usize::from(channels) * 256 => Ok(None),
        RequestFacts::EvalLut { .. } => {
            Err(PilError::ValueError("wrong number of lut entries".into()))
        }
        _ => Err(wrong_request("Eval")),
    }
}

fn assess_point_float_lut_request(
    contract: InputContract,
    request: RequestFacts,
) -> Result<Option<UnresolvedGuard>, PilError> {
    let InputContract::PointFloatLutEntries { channels } = contract else {
        unreachable!("float LUT request assessment received another contract");
    };
    match request {
        RequestFacts::PointFloatLut { entries } if entries == usize::from(channels) * 256 => {
            Ok(None)
        }
        RequestFacts::PointFloatLut { .. } => {
            Err(PilError::ValueError("wrong number of lut entries".into()))
        }
        _ => Err(wrong_request("PointFloat")),
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn assess_point_i32_lut_request(
    contract: InputContract,
    request: RequestFacts,
) -> Result<Option<UnresolvedGuard>, PilError> {
    let InputContract::PointI32LutEntries { entries: expected } = contract else {
        unreachable!("I32 point LUT assessment received another contract");
    };
    match request {
        RequestFacts::EvalLut { entries } if entries == expected => Ok(None),
        RequestFacts::EvalLut { .. } => {
            Err(PilError::ValueError("wrong number of lut entries".into()))
        }
        _ => Err(wrong_request("PointModeOutput")),
    }
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn assess_reduce_request(
    contract: InputContract,
    request: RequestFacts,
) -> Result<Option<UnresolvedGuard>, PilError> {
    let InputContract::ReduceFactors { identity_only } = contract else {
        unreachable!("Reduce request assessment received another contract");
    };
    let RequestFacts::Reduce { x_factor, y_factor } = request else {
        return Err(wrong_request("Reduce"));
    };
    if x_factor == 0 || y_factor == 0 {
        Err(PilError::ValueError("scale must be > 0".into()))
    } else if identity_only && (x_factor != 1 || y_factor != 1) {
        Err(PilError::ValueError("image has wrong mode".into()))
    } else {
        Ok(None)
    }
}

fn assess_thumbnail_request(input: StrategyInputs) -> Result<Option<UnresolvedGuard>, PilError> {
    let RequestFacts::Thumbnail {
        width,
        height,
        reducing_gap_bits,
        ..
    } = input.request
    else {
        return Err(wrong_request("Thumbnail"));
    };
    if width == 0 || height == 0 {
        Err(PilError::ValueError("thumbnail size must be > 0".into()))
    } else if input.width == 0 || input.height == 0 {
        Err(PilError::ValueError("height and width must be > 0".into()))
    } else if reducing_gap_bits.is_some_and(|bits| f64::from_bits(bits) < 1.0) {
        Err(PilError::ValueError(
            "reducing_gap must be 1.0 or greater".into(),
        ))
    } else {
        Ok(None)
    }
}

fn assess_imageops_request(
    contract: InputContract,
    request: RequestFacts,
) -> Result<Option<UnresolvedGuard>, PilError> {
    match (contract, request) {
        (InputContract::ContainParameters, RequestFacts::Contain { .. })
        | (InputContract::CoverParameters, RequestFacts::Cover { .. })
        | (InputContract::FitParameters, RequestFacts::Fit { .. })
        | (InputContract::PadParameters, RequestFacts::Pad { .. })
        | (InputContract::ScaleParameters, RequestFacts::Scale { .. }) => Ok(None),
        (InputContract::ContainParameters, _) => Err(wrong_request("Contain")),
        (InputContract::CoverParameters, _) => Err(wrong_request("Cover")),
        (InputContract::FitParameters, _) => Err(wrong_request("Fit")),
        (InputContract::PadParameters, _) => Err(wrong_request("Pad")),
        (InputContract::ScaleParameters, _) => Err(wrong_request("Scale")),
        _ => Err(PilError::InternalError(
            "ImageOps request assessment received another contract".into(),
        )),
    }
}

fn assess_rank_window(
    contract: InputContract,
    request: RequestFacts,
) -> Result<Option<UnresolvedGuard>, PilError> {
    let InputContract::OddRankWindow { rank_required } = contract else {
        unreachable!("rank-window assessment received another contract");
    };
    let RequestFacts::FilterWindow { size, rank, .. } = request else {
        return Err(wrong_request("RankFilter"));
    };
    let area = size
        .checked_mul(size)
        .filter(|_| size > 0 && !size.is_multiple_of(2))
        .ok_or_else(|| PilError::ValueError("bad filter size".into()))?;
    if rank_required && rank.is_none() {
        return Err(PilError::InternalError(
            "rank-filter contract requires a rank value".into(),
        ));
    }
    if rank.is_some_and(|value| value >= area) {
        return Err(PilError::ValueError("bad rank value".into()));
    }
    Ok(None)
}

fn assess_mode_filter_window(request: RequestFacts) -> Result<Option<UnresolvedGuard>, PilError> {
    let RequestFacts::FilterWindow { size, rank, .. } = request else {
        return Err(wrong_request("ModeFilter"));
    };
    if rank.is_some() {
        return Err(PilError::InternalError(
            "ModeFilter contract received rank-filter facts".into(),
        ));
    }
    size.checked_mul(size)
        .filter(|_| size == 0 || !size.is_multiple_of(2))
        .ok_or_else(|| PilError::ValueError("bad filter size".into()))?;
    Ok(None)
}
