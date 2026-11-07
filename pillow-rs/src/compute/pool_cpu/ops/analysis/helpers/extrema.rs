//! Native sample reductions shared by exact-mode GetExtrema strategies.

use crate::image::FormattedExtrema;
use crate::raster::{ImageBuffer, Pixel};

/// Reduce native byte channels without inspecting a mode tag or changing pixels.
pub(crate) fn bytes<P>(image: &ImageBuffer<P, Vec<u8>>) -> FormattedExtrema
where
    P: Pixel<Subpixel = u8>,
{
    let mut values = vec![(u8::MAX, 0); usize::from(P::CHANNEL_COUNT)];
    for pixel in image.pixels() {
        for (band, &sample) in pixel.channels().iter().enumerate() {
            values[band].0 = values[band].0.min(sample);
            values[band].1 = values[band].1.max(sample);
        }
    }
    if values.len() == 1 {
        FormattedExtrema::Single(values[0])
    } else {
        FormattedExtrema::Multiple(values)
    }
}

/// Reduce native u16 channels for exact CoreOnly multiband modes.
pub(crate) fn u16_bands<P>(image: &ImageBuffer<P, Vec<u16>>) -> FormattedExtrema
where
    P: Pixel<Subpixel = u16>,
{
    let mut values = vec![(u16::MAX, 0); usize::from(P::CHANNEL_COUNT)];
    for pixel in image.pixels() {
        for (band, &sample) in pixel.channels().iter().enumerate() {
            values[band].0 = values[band].0.min(sample);
            values[band].1 = values[band].1.max(sample);
        }
    }
    FormattedExtrema::MultipleInteger(
        values
            .into_iter()
            .map(|(minimum, maximum)| (i32::from(minimum), i32::from(maximum)))
            .collect(),
    )
}

/// Reduce native f32 channels for exact CoreOnly multiband modes.
///
/// Initialize from the first sample and keep Pillow C's ordered comparisons;
/// using `f32::min` would change the result when the first sample is NaN.
pub(crate) fn f32_bands<P>(image: &ImageBuffer<P, Vec<f32>>) -> FormattedExtrema
where
    P: Pixel<Subpixel = f32>,
{
    let mut pixels = image.pixels();
    let first = pixels
        .next()
        .expect("nonempty multiband float image has at least one pixel");
    let mut values: Vec<_> = first
        .channels()
        .iter()
        .map(|&sample| (sample, sample))
        .collect();
    for pixel in pixels {
        for (band, &sample) in pixel.channels().iter().enumerate() {
            if values[band].0 > sample {
                values[band].0 = sample;
            } else if values[band].1 < sample {
                values[band].1 = sample;
            }
        }
    }
    FormattedExtrema::MultipleFloat(
        values
            .into_iter()
            .map(|(minimum, maximum)| (f64::from(minimum), f64::from(maximum)))
            .collect(),
    )
}

/// Reduce Pillow's scalar `I` carrier using signed native samples.
pub(crate) fn i32_scalar(image: &crate::raster::RgbaImage) -> FormattedExtrema {
    let (minimum, maximum) = image
        .as_raw()
        .chunks_exact(4)
        .map(|sample| i32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]))
        .fold(None, |extrema: Option<(i32, i32)>, value| {
            Some(match extrema {
                Some((minimum, maximum)) => (minimum.min(value), maximum.max(value)),
                None => (value, value),
            })
        })
        .expect("nonempty I image has at least one scalar sample");
    FormattedExtrema::Integer((minimum, maximum))
}

/// Reduce Pillow's scalar `F` carrier using its native float32 values.
///
/// Preserve `ImagingGetExtrema`'s first-sample initialization and ordered
/// comparisons so NaN samples have the same behavior as Pillow.
pub(crate) fn f32_scalar(image: &crate::raster::RgbaImage) -> FormattedExtrema {
    let mut samples = image
        .as_raw()
        .chunks_exact(4)
        .map(|sample| f32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]));
    let first = samples
        .next()
        .expect("nonempty F image has at least one scalar sample");
    let (mut minimum, mut maximum) = (first, first);
    for value in samples {
        if minimum > value {
            minimum = value;
        } else if maximum < value {
            maximum = value;
        }
    }
    FormattedExtrema::Float((f64::from(minimum), f64::from(maximum)))
}

/// Reduce native unsigned 16-bit single-band samples.
pub(crate) fn u16_scalar(
    image: &crate::compute::pool_cpu::ops::analysis::helpers::get_data::Luma16Image,
) -> FormattedExtrema {
    let (minimum, maximum) = image
        .as_raw()
        .iter()
        .copied()
        .fold(None, |extrema: Option<(u16, u16)>, value| {
            Some(match extrema {
                Some((minimum, maximum)) => (minimum.min(value), maximum.max(value)),
                None => (value, value),
            })
        })
        .expect("nonempty I;16 image has at least one sample");
    FormattedExtrema::Integer((i32::from(minimum), i32::from(maximum)))
}

/// Generate the exact-mode architecture dispatcher for GetExtrema.
macro_rules! declare_getextrema_owner {
    ($mode:ident, $image_ty:ty, $implementation:path $(,)?) => {
        pub(crate) fn execute_getextrema(
            image: &$image_ty,
            selection: crate::compute::registry::support::StrategySelection,
        ) -> Result<crate::image::FormattedExtrema, crate::error::PilError> {
            let implementation = selection.implementation;
            if implementation.mode != crate::raster::Mode::$mode
                || implementation.operation
                    != crate::compute::registry::support::OperationId::GetExtrema
            {
                return Err(crate::error::PilError::InternalError(
                    "GetExtrema strategy selection did not match its operation and mode".into(),
                ));
            }
            match implementation.architecture {
                crate::compute::Architecture::Default => Ok($implementation(image)),
                architecture => Err(crate::error::PilError::InternalError(format!(
                    "GetExtrema has no {:?} {architecture:?} strategy",
                    crate::raster::Mode::$mode,
                ))),
            }
        }
    };
}

pub(crate) use declare_getextrema_owner;
