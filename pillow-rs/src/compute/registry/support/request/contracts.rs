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
        InputContract::EffectSpreadDistance
        | InputContract::EffectNoiseSigma
        | InputContract::Rotate
        | InputContract::PosterizeBits
        | InputContract::ByteLutEntries { .. }
        | InputContract::ReduceFactors { .. } => assess_request(contract, input),
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

fn assess_request(
    contract: InputContract,
    input: StrategyInputs,
) -> Result<Option<UnresolvedGuard>, PilError> {
    match (contract, input.request) {
        (InputContract::EffectSpreadDistance, RequestFacts::Unspecified)
        | (InputContract::EffectNoiseSigma, RequestFacts::Unspecified)
        | (InputContract::Rotate, RequestFacts::Unspecified)
        | (InputContract::PosterizeBits, RequestFacts::Unspecified)
        | (InputContract::ByteLutEntries { .. }, RequestFacts::Unspecified)
        | (InputContract::ReduceFactors { .. }, RequestFacts::Unspecified) => {
            unresolved_request(contract)
        }
        (InputContract::EffectSpreadDistance, RequestFacts::EffectSpread { .. })
        | (InputContract::EffectNoiseSigma, RequestFacts::EffectNoise { .. })
        | (InputContract::Rotate, RequestFacts::Rotate { .. }) => Ok(None),
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
        (InputContract::ReduceFactors { identity_only }, RequestFacts::Reduce { x_factor, y_factor }) => {
            if identity_only && (x_factor.max(1) != 1 || y_factor.max(1) != 1) {
                Err(PilError::ValueError("image has wrong mode".into()))
            } else {
                Ok(None)
            }
        }
        (InputContract::EffectSpreadDistance, _) => Err(wrong_request("EffectSpread")),
        (InputContract::EffectNoiseSigma, _) => Err(wrong_request("EffectNoise")),
        (InputContract::Rotate, _) => Err(wrong_request("Rotate")),
        (InputContract::ByteLutEntries { .. }, _) => Err(wrong_request("Eval")),
        (InputContract::ReduceFactors { .. }, _) => Err(wrong_request("Reduce")),
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
        RequestFacts::PutAlphaData { mask } if modes.contains(&mask.mode) => Ok(None),
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
