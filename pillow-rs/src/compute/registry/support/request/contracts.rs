//! Semantic request and input contracts for operation strategies.

use super::{
    InputContract, PaletteKnowledge, RequestFacts, StrategyInputs, UnresolvedGuard,
};
use crate::error::PilError;
use crate::raster::Mode;

pub(super) fn assess(
    contract: InputContract,
    input: StrategyInputs,
) -> Result<Option<UnresolvedGuard>, PilError> {
    match contract {
        InputContract::Filter3x3Parameters
        | InputContract::Filter5x5Parameters
        | InputContract::GaussianBlurSigma
        | InputContract::BoxBlurParameters
        | InputContract::UnsharpMaskParameters
        | InputContract::SharpnessFactor
        | InputContract::EffectSpreadDistance
        | InputContract::EffectNoiseSigma
        | InputContract::Rotate
        | InputContract::PosterizeBits
        | InputContract::ByteLutEntries { .. }
        | InputContract::PointFloatLutEntries { .. }
        | InputContract::ReduceFactors { .. }
        | InputContract::Thumbnail
        | InputContract::ContainParameters
        | InputContract::CoverParameters
        | InputContract::FitParameters
        | InputContract::PadParameters
        | InputContract::ScaleParameters
        | InputContract::OddRankWindow { .. } => assess_request(contract, input),
        InputContract::CheckedByteDimensions { bytes_per_pixel, allow_empty } => {
            assess_checked_dimensions(input, bytes_per_pixel, allow_empty)
        }
        InputContract::ExpandedDimensions { bytes_per_pixel } => {
            assess_expanded_dimensions(contract, input, bytes_per_pixel)
        }
        InputContract::PaletteMetadata if input.palette == PaletteKnowledge::Unknown => {
            Ok(Some(UnresolvedGuard::PaletteMetadata))
        }
        InputContract::Color3DLutTarget { targets } => assess_color3dlut_target(input, targets),
        InputContract::ConvertTarget {
            targets,
            core_only_targets,
            core_only_reason,
            rejection_reason,
        } => {
            assess_convert_target(
                input,
                targets,
                core_only_targets,
                core_only_reason,
                rejection_reason,
            )
        }
        InputContract::ChopsBinaryModes { modes, rejection_reason } => {
            assess_chops_binary_modes(input, modes, rejection_reason)
        }
        InputContract::SameModeSecondaryImage { mode, rejection_reason } => {
            assess_same_mode_secondary_image(input, mode, rejection_reason)
        }
        InputContract::CropBorder => assess_crop_border(contract, input),
        InputContract::ByteDataPayload { modes, rejection_reason } => {
            assess_byte_data_payload(contract, input, modes, rejection_reason)
        }
        InputContract::AlphaMaskLumaPrefix { modes, .. } => {
            assess_alpha_mask(contract, input, modes)
        }
        InputContract::ImageOpsLumaMask => assess_imageops_luma_mask(contract, input),
        InputContract::ExactNativeBytes { channels, overflow_reason, mismatch_reason } => {
            assess_exact_native_bytes(contract, input, channels, overflow_reason, mismatch_reason)
        }
        InputContract::PaletteMetadata
        | InputContract::NormalizedCrop
        | InputContract::ExtractBandIndex(_)
        | InputContract::CallerValidatedPixelCoordinates
        | InputContract::OrderedPixelWrites => Ok(None),
    }
}

fn assess_convert_target(
    input: StrategyInputs,
    targets: &'static [Mode],
    core_only_targets: &'static [Mode],
    core_only_reason: Option<&'static str>,
    rejection_reason: &'static str,
) -> Result<Option<UnresolvedGuard>, PilError> {
    match input.request {
        RequestFacts::Unspecified => Ok(Some(UnresolvedGuard::InputContract(
            InputContract::ConvertTarget {
                targets,
                core_only_targets,
                core_only_reason,
                rejection_reason,
            },
        ))),
        RequestFacts::ConvertTarget {
            target_mode: Mode::One | Mode::ImagePalette8,
        } => Err(PilError::ValueError(
            "deferred conversion mode must be materialized first".into(),
        )),
        RequestFacts::ConvertTarget { target_mode } if targets.contains(&target_mode) => Ok(None),
        RequestFacts::ConvertTarget { .. } => {
            Err(PilError::ValueError(rejection_reason.into()))
        }
        _ => Err(PilError::InternalError(
            "Convert target validation received another operation request".into(),
        )),
    }
}

fn assess_chops_binary_modes(
    input: StrategyInputs,
    modes: &'static [Mode],
    rejection_reason: &'static str,
) -> Result<Option<UnresolvedGuard>, PilError> {
    match input.request {
        RequestFacts::Unspecified => Ok(Some(UnresolvedGuard::InputContract(
            InputContract::ChopsBinaryModes { modes, rejection_reason },
        ))),
        RequestFacts::ChopsBinary { other, .. } if modes.contains(&other.mode) => Ok(None),
        RequestFacts::ChopsBinary { .. } => {
            Err(PilError::ValueError(rejection_reason.into()))
        }
        _ => Err(wrong_request("ImageChops binary operation")),
    }
}

fn assess_same_mode_secondary_image(
    input: StrategyInputs,
    mode: Mode,
    rejection_reason: &'static str,
) -> Result<Option<UnresolvedGuard>, PilError> {
    match input.request {
        RequestFacts::Unspecified => Ok(Some(UnresolvedGuard::InputContract(
            InputContract::SameModeSecondaryImage { mode, rejection_reason },
        ))),
        RequestFacts::SameModeSecondaryImage { other }
            if other.mode == mode && (other.width, other.height) == (input.width, input.height) =>
        {
            Ok(None)
        }
        RequestFacts::SameModeSecondaryImage { .. } => {
            Err(PilError::ValueError(rejection_reason.into()))
        }
        _ => Err(wrong_request("same-mode secondary image")),
    }
}

fn assess_request(
    contract: InputContract,
    input: StrategyInputs,
) -> Result<Option<UnresolvedGuard>, PilError> {
    match (contract, input.request) {
        (InputContract::Filter3x3Parameters, RequestFacts::Unspecified)
        | (InputContract::Filter5x5Parameters, RequestFacts::Unspecified)
        | (InputContract::GaussianBlurSigma, RequestFacts::Unspecified)
        | (InputContract::BoxBlurParameters, RequestFacts::Unspecified)
        | (InputContract::UnsharpMaskParameters, RequestFacts::Unspecified)
        | (InputContract::SharpnessFactor, RequestFacts::Unspecified)
        | (InputContract::EffectSpreadDistance, RequestFacts::Unspecified)
        | (InputContract::EffectNoiseSigma, RequestFacts::Unspecified)
        | (InputContract::Rotate, RequestFacts::Unspecified)
        | (InputContract::PosterizeBits, RequestFacts::Unspecified)
        | (InputContract::ByteLutEntries { .. }, RequestFacts::Unspecified)
        | (InputContract::PointFloatLutEntries { .. }, RequestFacts::Unspecified)
        | (InputContract::ReduceFactors { .. }, RequestFacts::Unspecified)
        | (InputContract::Thumbnail, RequestFacts::Unspecified)
        | (InputContract::ContainParameters, RequestFacts::Unspecified)
        | (InputContract::CoverParameters, RequestFacts::Unspecified)
        | (InputContract::FitParameters, RequestFacts::Unspecified)
        | (InputContract::PadParameters, RequestFacts::Unspecified)
        | (InputContract::ScaleParameters, RequestFacts::Unspecified)
        | (InputContract::OddRankWindow { .. }, RequestFacts::Unspecified) => {
            unresolved_request(contract)
        }
        (InputContract::EffectSpreadDistance, RequestFacts::EffectSpread { .. })
        | (InputContract::EffectNoiseSigma, RequestFacts::EffectNoise { .. })
        | (InputContract::Rotate, RequestFacts::Rotate { .. }) => Ok(None),
        (InputContract::Filter3x3Parameters, RequestFacts::Filter3x3 { .. })
        | (InputContract::Filter5x5Parameters, RequestFacts::Filter5x5 { .. })
        | (InputContract::GaussianBlurSigma, RequestFacts::GaussianBlur { .. })
        | (
            InputContract::BoxBlurParameters,
            RequestFacts::BoxBlur { .. } | RequestFacts::BoxBlurXY { .. },
        )
        | (InputContract::UnsharpMaskParameters, RequestFacts::UnsharpMask { .. }) => Ok(None),
        (InputContract::SharpnessFactor, RequestFacts::Sharpness { .. }) => Ok(None),
        (InputContract::PosterizeBits, RequestFacts::Posterize { bits })
            if (1..=8).contains(&bits) => Ok(None),
        (InputContract::PosterizeBits, _) => Err(PilError::ValueError(
            "posterize bits must be in range 1..=8".into(),
        )),
        (InputContract::ByteLutEntries { channels }, RequestFacts::EvalLut { entries })
            if entries == usize::from(channels) * 256 => Ok(None),
        (InputContract::ByteLutEntries { .. }, RequestFacts::EvalLut { .. }) => {
            Err(PilError::ValueError("wrong number of lut entries".into()))
        }
        (
            InputContract::PointFloatLutEntries { channels },
            RequestFacts::PointFloatLut { entries },
        ) if entries == usize::from(channels) * 256 => Ok(None),
        (InputContract::PointFloatLutEntries { .. }, RequestFacts::PointFloatLut { .. }) => {
            Err(PilError::ValueError("wrong number of lut entries".into()))
        }
        (InputContract::ReduceFactors { identity_only }, RequestFacts::Reduce { x_factor, y_factor }) => {
            if identity_only && (x_factor.max(1) != 1 || y_factor.max(1) != 1) {
                Err(PilError::ValueError("image has wrong mode".into()))
            } else {
                Ok(None)
            }
        }
        (InputContract::Thumbnail, RequestFacts::Thumbnail { width, height, .. }) => {
            if width == 0 || height == 0 {
                Err(PilError::ValueError("thumbnail size must be > 0".into()))
            } else if input.width == 0 || input.height == 0 {
                Err(PilError::ValueError("height and width must be > 0".into()))
            } else {
                Ok(None)
            }
        }
        (InputContract::ContainParameters, RequestFacts::Contain { .. })
        | (InputContract::CoverParameters, RequestFacts::Cover { .. })
        | (InputContract::FitParameters, RequestFacts::Fit { .. })
        | (InputContract::PadParameters, RequestFacts::Pad { .. })
        | (InputContract::ScaleParameters, RequestFacts::Scale { .. }) => Ok(None),
        (
            InputContract::OddRankWindow { rank_required },
            RequestFacts::FilterWindow { size, rank },
        ) => {
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
        (InputContract::EffectSpreadDistance, _) => Err(wrong_request("EffectSpread")),
        (InputContract::Filter3x3Parameters, _) => Err(wrong_request("Filter3x3")),
        (InputContract::Filter5x5Parameters, _) => Err(wrong_request("Filter5x5")),
        (InputContract::GaussianBlurSigma, _) => Err(wrong_request("GaussianBlur")),
        (InputContract::BoxBlurParameters, _) => Err(wrong_request("BoxBlur")),
        (InputContract::UnsharpMaskParameters, _) => Err(wrong_request("UnsharpMask")),
        (InputContract::SharpnessFactor, _) => Err(wrong_request("Sharpness")),
        (InputContract::EffectNoiseSigma, _) => Err(wrong_request("EffectNoise")),
        (InputContract::Rotate, _) => Err(wrong_request("Rotate")),
        (InputContract::ByteLutEntries { .. }, _) => Err(wrong_request("Eval")),
        (InputContract::PointFloatLutEntries { .. }, _) => Err(wrong_request("PointFloat")),
        (InputContract::ReduceFactors { .. }, _) => Err(wrong_request("Reduce")),
        (InputContract::Thumbnail, _) => Err(wrong_request("Thumbnail")),
        (InputContract::ContainParameters, _) => Err(wrong_request("Contain")),
        (InputContract::CoverParameters, _) => Err(wrong_request("Cover")),
        (InputContract::FitParameters, _) => Err(wrong_request("Fit")),
        (InputContract::PadParameters, _) => Err(wrong_request("Pad")),
        (InputContract::ScaleParameters, _) => Err(wrong_request("Scale")),
        (InputContract::OddRankWindow { .. }, _) => Err(wrong_request("RankFilter")),
        _ => Err(PilError::InternalError(
            "request validation received a non-request input contract".into(),
        )),
    }
}

fn assess_checked_dimensions(
    input: StrategyInputs,
    bytes_per_pixel: u8,
    allow_empty: bool,
) -> Result<Option<UnresolvedGuard>, PilError> {
    if allow_empty {
        crate::checked_dims::CheckedDims::new_allow_empty(
            input.width,
            input.height,
            bytes_per_pixel,
        )?;
    } else {
        crate::checked_dims::CheckedDims::new(input.width, input.height, bytes_per_pixel)?;
    }
    Ok(None)
}

fn assess_expanded_dimensions(
    contract: InputContract,
    input: StrategyInputs,
    bytes_per_pixel: u8,
) -> Result<Option<UnresolvedGuard>, PilError> {
    let border = match input.request {
        RequestFacts::Unspecified => return unresolved_request(contract),
        RequestFacts::Expand { border } => border,
        _ => return Err(wrong_request("Expand")),
    };
    let source = crate::checked_dims::CheckedDims::new_allow_empty(
        input.width,
        input.height,
        bytes_per_pixel,
    )?;
    match input.byte_len {
        None => return unresolved_request(contract),
        Some(length) if length != source.total_bytes() => {
            return Err(PilError::InternalError(
                "Expand source byte length did not match its native mode".into(),
            ));
        }
        Some(_) => (),
    }
    let (width, height) = expanded_dimensions(input.width, input.height, border)?;
    crate::checked_dims::CheckedDims::new(width, height, bytes_per_pixel)?;
    Ok(None)
}

fn expanded_dimensions(width: u32, height: u32, border: u32) -> Result<(u32, u32), PilError> {
    let border_twice = border.checked_mul(2).ok_or_else(|| {
        PilError::DimensionError("expand border overflows dimensions".into())
    })?;
    let width = width.checked_add(border_twice).ok_or_else(|| {
        PilError::DimensionError("expanded image width overflows".into())
    })?;
    let height = height.checked_add(border_twice).ok_or_else(|| {
        PilError::DimensionError("expanded image height overflows".into())
    })?;
    Ok((width, height))
}

fn assess_color3dlut_target(
    input: StrategyInputs,
    targets: &'static [super::ModeChannelCapacity],
) -> Result<Option<UnresolvedGuard>, PilError> {
    match input.request {
        RequestFacts::Unspecified => Ok(Some(UnresolvedGuard::InputContract(
            InputContract::Color3DLutTarget { targets },
        ))),
        RequestFacts::Color3DLut { target_mode, channels, .. }
            if targets.iter().any(|target| {
                target.mode == target_mode && channels <= u32::from(target.channels)
            }) => Ok(None),
        _ => Err(PilError::ValueError("image has wrong mode".into())),
    }
}

fn assess_crop_border(
    contract: InputContract,
    input: StrategyInputs,
) -> Result<Option<UnresolvedGuard>, PilError> {
    let border = match input.request {
        RequestFacts::Unspecified => return unresolved_request(contract),
        RequestFacts::CropBorder { border } => border,
        _ => return Err(wrong_request("CropBorder")),
    };
    if border > input.width / 2 {
        return Err(PilError::ValueError("Coordinate 'right' is less than 'left'".into()));
    }
    if border > input.height / 2 {
        return Err(PilError::ValueError("Coordinate 'lower' is less than 'upper'".into()));
    }
    Ok(None)
}

fn assess_byte_data_payload(
    contract: InputContract,
    input: StrategyInputs,
    modes: &'static [Mode],
    rejection_reason: &'static str,
) -> Result<Option<UnresolvedGuard>, PilError> {
    match input.request {
        RequestFacts::Unspecified => unresolved_request(contract),
        RequestFacts::PutData { mode, .. } if modes.contains(&mode) => Ok(None),
        _ => Err(PilError::ValueError(rejection_reason.into())),
    }
}

fn assess_alpha_mask(
    contract: InputContract,
    input: StrategyInputs,
    modes: &'static [Mode],
) -> Result<Option<UnresolvedGuard>, PilError> {
    match input.request {
        RequestFacts::Unspecified => unresolved_request(contract),
        RequestFacts::PutAlphaData { mask }
            if modes.contains(&mask.mode)
                && (mask.width, mask.height) == (input.width, input.height) =>
        {
            Ok(None)
        }
        RequestFacts::PutAlphaData { mask } if modes.contains(&mask.mode) => {
            Err(PilError::ValueError("images do not match".into()))
        }
        _ => Err(PilError::ValueError("illegal image mode".into())),
    }
}

fn assess_imageops_luma_mask(
    contract: InputContract,
    input: StrategyInputs,
) -> Result<Option<UnresolvedGuard>, PilError> {
    match input.request {
        RequestFacts::Unspecified => unresolved_request(contract),
        RequestFacts::ImageOpsMask { mask: None } => Ok(None),
        RequestFacts::ImageOpsMask { mask: Some(mask) } => {
            validate_imageops_mask(input, mask.mode, mask.width, mask.height)?;
            Ok(None)
        }
        _ => Err(wrong_request("ImageOps histogram")),
    }
}

fn validate_imageops_mask(
    input: StrategyInputs,
    mode: Mode,
    width: u32,
    height: u32,
) -> Result<(), PilError> {
    if !matches!(mode, Mode::One | Mode::ImageLuma8) {
        return Err(PilError::ValueError("bad transparency mask".into()));
    }
    if (width, height) != (input.width, input.height) {
        return Err(PilError::ValueError("images do not match".into()));
    }
    Ok(())
}

fn assess_exact_native_bytes(
    contract: InputContract,
    input: StrategyInputs,
    channels: u8,
    overflow_reason: &'static str,
    mismatch_reason: &'static str,
) -> Result<Option<UnresolvedGuard>, PilError> {
    let expected = (input.width as usize)
        .checked_mul(input.height as usize)
        .and_then(|pixels| pixels.checked_mul(usize::from(channels)))
        .ok_or_else(|| PilError::InternalError(overflow_reason.into()))?;
    match input.byte_len {
        None => unresolved_request(contract),
        Some(length) if length != expected => {
            Err(PilError::InternalError(mismatch_reason.into()))
        }
        Some(_) => Ok(None),
    }
}

fn unresolved_request(
    contract: InputContract,
) -> Result<Option<UnresolvedGuard>, PilError> {
    Ok(Some(UnresolvedGuard::InputContract(contract)))
}

fn wrong_request(operation: &str) -> PilError {
    PilError::InternalError(format!("{operation} support received facts for another operation"))
}
