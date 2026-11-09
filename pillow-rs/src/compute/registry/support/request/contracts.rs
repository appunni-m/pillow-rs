//! Semantic request and input contracts for operation strategies.

use super::{
    DrawRequestKind, DrawTextMaskKind, InputContract, MergeBandModeRule, PaletteKnowledge,
    PasteInputRules, PasteMaskRule, RequestFacts, StrategyInputs, UnresolvedGuard,
};
use crate::error::PilError;
use crate::pipeline::ExpandMargins;
use crate::raster::Mode;

mod request;

pub(super) fn assess(
    contract: InputContract,
    input: StrategyInputs,
) -> Result<Option<UnresolvedGuard>, PilError> {
    match contract {
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
        | InputContract::PointFloatLutEntries { .. }
        | InputContract::ReduceFactors { .. }
        | InputContract::Thumbnail
        | InputContract::ContainParameters
        | InputContract::CoverParameters
        | InputContract::FitParameters
        | InputContract::PadParameters
        | InputContract::ScaleParameters
        | InputContract::OddRankWindow { .. }
        | InputContract::ModeFilterWindow => request::assess(contract, input),
        InputContract::MergeBandLayout { .. } => assess_merge_band_layout(contract, input),
        InputContract::BlendInputs { .. } => assess_blend_inputs(contract, input),
        InputContract::CompositeInputs { .. } => assess_composite_inputs(contract, input),
        InputContract::CheckedByteDimensions {
            bytes_per_pixel,
            allow_empty,
        } => assess_checked_dimensions(input, bytes_per_pixel, allow_empty),
        InputContract::ExpandedDimensions { bytes_per_pixel } => {
            assess_expanded_dimensions(contract, input, bytes_per_pixel)
        }
        InputContract::ExpandFill(_)
        | InputContract::PadFill(_)
        | InputContract::AutocontrastParameters
        | InputContract::FloodFillInputs => Ok(None),
        InputContract::PaletteMetadata if input.palette == PaletteKnowledge::Unknown => {
            Ok(Some(UnresolvedGuard::PaletteMetadata))
        }
        InputContract::Color3DLutTarget { targets } => assess_color3dlut_target(input, targets),
        InputContract::ConvertTarget {
            targets,
            core_only_targets,
            core_only_reason,
            rejection_reason,
        } => assess_convert_target(
            input,
            targets,
            core_only_targets,
            core_only_reason,
            rejection_reason,
        ),
        contract @ InputContract::ChopsBinaryModes { .. } => {
            assess_chops_binary_modes(input, contract)
        }
        InputContract::SameModeSecondaryImage {
            mode,
            rejection_reason,
        } => assess_same_mode_secondary_image(input, mode, rejection_reason),
        contract @ InputContract::DrawInputs { .. } => assess_draw_inputs(input, contract),
        InputContract::PasteInputs { rules } => assess_paste_inputs(input, rules),
        InputContract::CropBorder => assess_crop_border(contract, input),
        InputContract::ByteDataPayload {
            modes,
            rejection_reason,
        } => assess_byte_data_payload(contract, input, modes, rejection_reason),
        InputContract::AlphaMaskLumaPrefix { modes, .. } => {
            assess_alpha_mask(contract, input, modes)
        }
        InputContract::ImageOpsLumaMask => assess_imageops_luma_mask(contract, input),
        InputContract::ExactNativeBytes {
            channels,
            overflow_reason,
            mismatch_reason,
        } => assess_exact_native_bytes(contract, input, channels, overflow_reason, mismatch_reason),
        // These variants duplicate architecture guards and must not be
        // declared as public input validation contracts.
        InputContract::PaletteMetadata
        | InputContract::NormalizedCrop
        | InputContract::ExtractBandIndex(_)
        | InputContract::CallerValidatedPixelCoordinates
        | InputContract::CallerValidatedPixelIndex
        | InputContract::CallerValidatedRawI16ByteRows
        | InputContract::CallerValidatedGetDataBandIndex { .. }
        | InputContract::CallerValidatedGetChannelIndex { .. }
        | InputContract::CallerValidatedBandIndex
        | InputContract::CallerValidatedSameModeSameSizeBandImage
        | InputContract::CallerValidatedSameSizeByteSingleBandImage
        | InputContract::CallerValidatedSetModeTargets { .. }
        | InputContract::GetBBoxAlphaOnly { .. }
        | InputContract::HistogramMask { .. }
        | InputContract::HistogramExtrema { .. }
        | InputContract::OrderedPixelWrites => Ok(None),
    }
}

fn assess_composite_inputs(
    contract: InputContract,
    input: StrategyInputs,
) -> Result<Option<UnresolvedGuard>, PilError> {
    let InputContract::CompositeInputs {
        luma_masks,
        alpha_masks,
        mode_rejection,
        mask_rejection,
        size_rejection,
    } = contract
    else {
        return Err(PilError::InternalError(
            "Composite assessment received another input contract".into(),
        ));
    };
    let RequestFacts::CompositeInputs {
        source_mode,
        destination,
        mask,
        mask_alpha,
    } = input.request
    else {
        if input.request == RequestFacts::Unspecified {
            return Ok(Some(UnresolvedGuard::InputContract(Box::new(contract))));
        }
        return Err(wrong_request("CompositeModule"));
    };
    if destination.mode != source_mode {
        return Err(PilError::ValueError(mode_rejection.into()));
    }
    let valid_mask = if mask_alpha {
        alpha_masks.contains(&mask.mode)
    } else {
        luma_masks.contains(&mask.mode)
    };
    if !valid_mask {
        return Err(PilError::ValueError(mask_rejection.into()));
    }
    if (mask.width, mask.height) != (input.width, input.height) {
        return Err(PilError::ValueError(size_rejection.into()));
    }
    Ok(None)
}

fn assess_blend_inputs(
    contract: InputContract,
    input: StrategyInputs,
) -> Result<Option<UnresolvedGuard>, PilError> {
    let InputContract::BlendInputs {
        compatible_modes,
        wrong_mode_modes,
        wrong_mode_rejection,
        mismatch_rejection,
    } = contract
    else {
        return Err(PilError::InternalError(
            "Blend assessment received another input contract".into(),
        ));
    };
    let RequestFacts::BlendInputs { other, .. } = input.request else {
        if input.request == RequestFacts::Unspecified {
            return Ok(Some(UnresolvedGuard::InputContract(Box::new(contract))));
        }
        return Err(wrong_request("Blend"));
    };
    if !compatible_modes.contains(&other.mode) {
        let reason = if wrong_mode_modes.contains(&other.mode) {
            wrong_mode_rejection
        } else {
            mismatch_rejection
        };
        return Err(PilError::ValueError(reason.into()));
    }
    if (other.width, other.height) != (input.width, input.height) {
        return Err(PilError::ValueError(mismatch_rejection.into()));
    }
    Ok(None)
}

fn assess_merge_band_layout(
    contract: InputContract,
    input: StrategyInputs,
) -> Result<Option<UnresolvedGuard>, PilError> {
    let (
        expected_count,
        mode_rule,
        count_rejection,
        first_mode_rejection,
        mode_rejection,
        size_rejection,
    ) = match contract {
        InputContract::MergeBandLayout {
            band_count,
            mode_rule,
            count_rejection,
            first_mode_rejection,
            mode_rejection,
            size_rejection,
        } => (
            band_count,
            mode_rule,
            count_rejection,
            first_mode_rejection,
            mode_rejection,
            size_rejection,
        ),
        _ => {
            return Err(PilError::InternalError(
                "Merge band assessment received another input contract".into(),
            ));
        }
    };
    let RequestFacts::MergeBands {
        target_mode,
        band_count,
        first_mode,
        remaining_modes_are_luma,
        dimensions_match,
    } = input.request
    else {
        if input.request == RequestFacts::Unspecified {
            return Ok(Some(UnresolvedGuard::InputContract(Box::new(contract))));
        }
        return Err(PilError::InternalError(
            "Merge input assessment received another operation request".into(),
        ));
    };
    if band_count != expected_count {
        return Err(PilError::ValueError(count_rejection.into()));
    }
    let later_modes_are_valid = match mode_rule {
        MergeBandModeRule::ExactTarget => {
            if first_mode != target_mode {
                return Err(PilError::ValueError(first_mode_rejection.into()));
            }
            true
        }
        MergeBandModeRule::LumaOrFirstPalette => {
            if !matches!(
                first_mode,
                Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8
            ) {
                return Err(PilError::ValueError(first_mode_rejection.into()));
            }
            remaining_modes_are_luma
        }
    };
    if !later_modes_are_valid {
        return Err(PilError::ValueError(mode_rejection.into()));
    }
    if !dimensions_match {
        return Err(PilError::ValueError(size_rejection.into()));
    }
    Ok(None)
}

fn assess_paste_inputs(
    input: StrategyInputs,
    rules: &'static PasteInputRules,
) -> Result<Option<UnresolvedGuard>, PilError> {
    let RequestFacts::Paste {
        source,
        mask,
        region,
        mask_alpha,
        ..
    } = input.request
    else {
        if input.request == RequestFacts::Unspecified {
            return Ok(Some(UnresolvedGuard::InputContract(Box::new(
                InputContract::PasteInputs { rules },
            ))));
        }
        return Err(wrong_request("Paste"));
    };

    if !rules.source_modes.contains(&source.mode)
        || (mask.is_some() && rules.unmasked_only_sources.contains(&source.mode))
    {
        return Err(PilError::ValueError(rules.source_rejection.into()));
    }
    if let Some(mask) = mask {
        let valid_mask = rules
            .masks
            .iter()
            .any(|rule: &PasteMaskRule| rule.mode == mask.mode && rule.alpha == mask_alpha);
        if !valid_mask {
            return Err(PilError::ValueError(rules.mask_rejection.into()));
        }
        if (mask.width, mask.height) != region {
            return Err(PilError::ValueError(rules.source_rejection.into()));
        }
    } else if mask_alpha {
        return Err(PilError::ValueError(rules.mask_rejection.into()));
    }

    if (source.width, source.height) == region {
        return Ok(None);
    }
    let compact = source.width == 1
        && source.height == 1
        && mask.is_some_and(|mask| {
            !mask_alpha
                && rules.compact_sources.contains(&source.mode)
                && rules.compact_masks.contains(&mask.mode)
                && (mask.width, mask.height) == region
        });
    if compact {
        Ok(None)
    } else {
        Err(PilError::ValueError(rules.source_rejection.into()))
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
        RequestFacts::Unspecified => Ok(Some(UnresolvedGuard::InputContract(Box::new(
            InputContract::ConvertTarget {
                targets,
                core_only_targets,
                core_only_reason,
                rejection_reason,
            },
        )))),
        RequestFacts::ConvertTarget { target_mode } if targets.contains(&target_mode) => Ok(None),
        RequestFacts::ConvertTarget { .. } => Err(PilError::ValueError(rejection_reason.into())),
        _ => Err(PilError::InternalError(
            "Convert target validation received another operation request".into(),
        )),
    }
}

fn assess_chops_binary_modes(
    input: StrategyInputs,
    contract: InputContract,
) -> Result<Option<UnresolvedGuard>, PilError> {
    let InputContract::ChopsBinaryModes {
        modes,
        rejection_reason,
        ..
    } = contract
    else {
        return Err(PilError::InternalError(
            "Chops mode assessment received another input contract".into(),
        ));
    };
    match input.request {
        RequestFacts::Unspecified => Ok(Some(UnresolvedGuard::InputContract(Box::new(contract)))),
        RequestFacts::ChopsBinary { other, .. } if modes.contains(&other.mode) => Ok(None),
        RequestFacts::ChopsBinary { .. } => Err(PilError::ValueError(rejection_reason.into())),
        _ => Err(wrong_request("ImageChops binary operation")),
    }
}

fn assess_same_mode_secondary_image(
    input: StrategyInputs,
    mode: Mode,
    rejection_reason: &'static str,
) -> Result<Option<UnresolvedGuard>, PilError> {
    match input.request {
        RequestFacts::Unspecified => Ok(Some(UnresolvedGuard::InputContract(Box::new(
            InputContract::SameModeSecondaryImage {
                mode,
                rejection_reason,
            },
        )))),
        RequestFacts::SameModeSecondaryImage { other }
            if other.mode == mode && (other.width, other.height) == (input.width, input.height) =>
        {
            Ok(None)
        }
        RequestFacts::SameModeSecondaryImage { .. } => {
            Err(PilError::ValueError(rejection_reason.into()))
        }
        RequestFacts::AlphaComposite { source, .. }
            if source.mode == mode
                && (source.width, source.height) == (input.width, input.height) =>
        {
            Ok(None)
        }
        RequestFacts::AlphaComposite { .. } => Err(PilError::ValueError(rejection_reason.into())),
        _ => Err(wrong_request("same-mode secondary image")),
    }
}

fn assess_draw_inputs(
    input: StrategyInputs,
    contract: InputContract,
) -> Result<Option<UnresolvedGuard>, PilError> {
    let InputContract::DrawInputs { palette_modes, .. } = contract else {
        unreachable!("Draw input assessment received another contract")
    };
    let RequestFacts::Draw {
        kind,
        raster_mode,
        context_mode,
        bitmap_mask_mode,
        embedded_color,
        text_mask_kind,
        text_ink,
        ..
    } = input.request
    else {
        return if input.request == RequestFacts::Unspecified {
            defer_draw_inputs(contract)
        } else {
            Err(wrong_request("Draw"))
        };
    };

    if matches!(kind, DrawRequestKind::Bitmap | DrawRequestKind::Text)
        && raster_mode.is_some_and(|mode| palette_modes.contains(&mode))
        && input.palette == PaletteKnowledge::Unknown
    {
        return Ok(Some(UnresolvedGuard::PaletteMetadata));
    }

    match kind {
        DrawRequestKind::GeometryBatch => Ok(None),
        DrawRequestKind::Bitmap => {
            assess_draw_bitmap(raster_mode, context_mode, bitmap_mask_mode, contract)
        }
        DrawRequestKind::Text => assess_draw_text(
            raster_mode,
            context_mode,
            embedded_color,
            text_mask_kind,
            text_ink,
            contract,
        ),
    }
}

fn assess_draw_bitmap(
    raster_mode: Option<Mode>,
    context_mode: Option<Mode>,
    mask_mode: Option<Mode>,
    contract: InputContract,
) -> Result<Option<UnresolvedGuard>, PilError> {
    let InputContract::DrawInputs {
        bitmap_masks,
        allow_rgba_context_on_rgb,
        mode_rejection,
        mask_rejection,
        ..
    } = contract
    else {
        unreachable!("Draw Bitmap assessment received another contract")
    };
    let (Some(raster_mode), Some(context_mode), Some(mask_mode)) =
        (raster_mode, context_mode, mask_mode)
    else {
        return defer_draw_inputs(contract);
    };
    if !bitmap_masks.contains(&mask_mode) {
        return Err(PilError::ValueError(mask_rejection.into()));
    }
    if context_mode != raster_mode
        && !(allow_rgba_context_on_rgb
            && raster_mode == Mode::ImageRgb8
            && context_mode == Mode::ImageRgba8)
    {
        return Err(PilError::ValueError(mode_rejection.into()));
    }
    Ok(None)
}

fn assess_draw_text(
    raster_mode: Option<Mode>,
    context_mode: Option<Mode>,
    embedded_color: Option<bool>,
    text_mask_kind: Option<DrawTextMaskKind>,
    text_ink: Option<i64>,
    contract: InputContract,
) -> Result<Option<UnresolvedGuard>, PilError> {
    let InputContract::DrawInputs {
        text_mask_kinds,
        allow_rgba_context_on_rgb,
        embedded_color_modes,
        mode_rejection,
        embedded_color_rejection,
        ..
    } = contract
    else {
        unreachable!("Draw Text assessment received another contract")
    };
    let (Some(raster_mode), Some(context_mode), Some(embedded_color), Some(text_mask_kind)) =
        (raster_mode, context_mode, embedded_color, text_mask_kind)
    else {
        return defer_draw_inputs(contract);
    };
    if !text_mask_kinds.contains(&text_mask_kind) {
        return Err(PilError::InternalError(
            "Draw Text produced an undeclared FreeType mask representation".into(),
        ));
    }
    if text_mask_kind == DrawTextMaskKind::Rgba && text_ink.is_none() {
        return Err(PilError::InternalError(
            "RGBA Draw Text mask has no foreground ink".into(),
        ));
    }
    if context_mode != raster_mode
        && !(allow_rgba_context_on_rgb
            && raster_mode == Mode::ImageRgb8
            && context_mode == Mode::ImageRgba8)
    {
        return Err(PilError::ValueError(mode_rejection.into()));
    }
    if embedded_color && !embedded_color_modes.contains(&context_mode) {
        return Err(PilError::ValueError(embedded_color_rejection.into()));
    }
    Ok(None)
}

fn defer_draw_inputs(contract: InputContract) -> Result<Option<UnresolvedGuard>, PilError> {
    Ok(Some(UnresolvedGuard::InputContract(Box::new(contract))))
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

fn expanded_dimensions(
    width: u32,
    height: u32,
    border: ExpandMargins,
) -> Result<(u32, u32), PilError> {
    let width = i128::from(width) + i128::from(border.left) + i128::from(border.right);
    let height = i128::from(height) + i128::from(border.top) + i128::from(border.bottom);
    let width = u32::try_from(width)
        .map_err(|_| PilError::DimensionError("expanded image width is invalid".into()))?;
    let height = u32::try_from(height)
        .map_err(|_| PilError::DimensionError("expanded image height is invalid".into()))?;
    Ok((width, height))
}

fn assess_color3dlut_target(
    input: StrategyInputs,
    targets: &'static [super::ModeChannelCapacity],
) -> Result<Option<UnresolvedGuard>, PilError> {
    match input.request {
        RequestFacts::Unspecified => Ok(Some(UnresolvedGuard::InputContract(Box::new(
            InputContract::Color3DLutTarget { targets },
        )))),
        RequestFacts::Color3DLut {
            target_mode,
            channels,
            ..
        } => {
            let Some(target) = targets.iter().find(|target| target.mode == target_mode) else {
                return Err(PilError::ValueError("unrecognized image mode".into()));
            };
            if channels > u32::from(target.channels) {
                return Err(PilError::ValueError("image has wrong mode".into()));
            }
            Ok(None)
        }
        _ => Err(wrong_request("Color3DLut")),
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
        return Err(PilError::ValueError(
            "Coordinate 'right' is less than 'left'".into(),
        ));
    }
    if border > input.height / 2 {
        return Err(PilError::ValueError(
            "Coordinate 'lower' is less than 'upper'".into(),
        ));
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
        RequestFacts::PutData { payload_mode, .. } if modes.contains(&payload_mode) => Ok(None),
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
        Some(length) if length != expected => Err(PilError::InternalError(mismatch_reason.into())),
        Some(_) => Ok(None),
    }
}

fn unresolved_request(contract: InputContract) -> Result<Option<UnresolvedGuard>, PilError> {
    Ok(Some(UnresolvedGuard::InputContract(Box::new(contract))))
}

fn wrong_request(operation: &str) -> PilError {
    PilError::InternalError(format!(
        "{operation} support received facts for another operation"
    ))
}
