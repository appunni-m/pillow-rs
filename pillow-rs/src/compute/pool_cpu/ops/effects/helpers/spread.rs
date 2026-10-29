//! Native-sample relocation for EffectSpread.

use crate::compute::helpers::effects::effect_spread_indices;
use crate::error::PilError;
use crate::raster::{ImageBuffer, Pixel};

pub(crate) struct Kernel<'a, P: Pixel> {
    input: &'a ImageBuffer<P, Vec<P::Subpixel>>,
    distance: u32,
}

pub(crate) fn prepare<'a, P: Pixel>(
    input: &'a ImageBuffer<P, Vec<P::Subpixel>>,
    distance: u32,
) -> Kernel<'a, P> {
    Kernel { input, distance }
}

pub(crate) fn execute<P: Pixel>(
    kernel: Kernel<'_, P>,
) -> Result<ImageBuffer<P, Vec<P::Subpixel>>, PilError> {
    if kernel.distance == 0 {
        return Ok(kernel.input.clone());
    }
    if kernel.distance == 1 && kernel.input.width() != 0 && kernel.input.height() != 0 {
        i32::try_from(kernel.input.width())
            .map_err(|_| PilError::ValueError("effect_spread width is too large".into()))?;
        i32::try_from(kernel.input.height())
            .map_err(|_| PilError::ValueError("effect_spread height is too large".into()))?;
        crate::compute::helpers::effects::effect_spread_advance_identity_rng(
            kernel.input.width(),
            kernel.input.height(),
        )?;
        return Ok(kernel.input.clone());
    }

    let mapping = effect_spread_indices(
        kernel.input.width(),
        kernel.input.height(),
        kernel.distance,
    )?;
    gather_native_samples(kernel.input, &mapping)
}

fn gather_native_samples<P: Pixel>(
    input: &ImageBuffer<P, Vec<P::Subpixel>>,
    mapping: &[usize],
) -> Result<ImageBuffer<P, Vec<P::Subpixel>>, PilError> {
    let channels = usize::from(P::CHANNEL_COUNT);
    let expected_samples = mapping
        .len()
        .checked_mul(channels)
        .ok_or_else(|| PilError::ValueError("effect_spread sample count overflow".into()))?;
    let source = input.as_raw();
    if source.len() < expected_samples {
        return Err(PilError::InternalError(
            "effect_spread source buffer was shorter than its dimensions".into(),
        ));
    }

    let mut output = vec![P::Subpixel::default(); source.len()];
    for (destination, &source_pixel) in mapping.iter().enumerate() {
        let destination_start = destination.checked_mul(channels).ok_or_else(|| {
            PilError::ValueError("effect_spread destination sample offset overflow".into())
        })?;
        let source_start = source_pixel.checked_mul(channels).ok_or_else(|| {
            PilError::ValueError("effect_spread source sample offset overflow".into())
        })?;
        let destination_end = destination_start.checked_add(channels).ok_or_else(|| {
            PilError::ValueError("effect_spread destination sample range overflow".into())
        })?;
        let source_end = source_start.checked_add(channels).ok_or_else(|| {
            PilError::ValueError("effect_spread source sample range overflow".into())
        })?;
        let Some(destination_samples) = output.get_mut(destination_start..destination_end) else {
            return Err(PilError::InternalError(
                "effect_spread destination mapping was out of bounds".into(),
            ));
        };
        let Some(source_samples) = source.get(source_start..source_end) else {
            return Err(PilError::InternalError(
                "effect_spread source mapping was out of bounds".into(),
            ));
        };
        destination_samples.copy_from_slice(source_samples);
    }

    ImageBuffer::from_raw(input.width(), input.height(), output).ok_or_else(|| {
        PilError::InternalError("effect_spread output sample count was invalid".into())
    })
}
