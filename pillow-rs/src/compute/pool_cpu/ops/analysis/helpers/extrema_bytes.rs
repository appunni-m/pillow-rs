//! Typed byte-result reduction kernels shared by exact-mode extrema strategies.

use crate::raster::color::FromPrimitive;
use crate::raster::{ImageBuffer, Pixel};

/// Returns the complete native samples belonging to the declared image extent.
///
pub(crate) fn logical_samples<P>(image: &ImageBuffer<P, Vec<P::Subpixel>>) -> &[P::Subpixel]
where
    P: Pixel,
{
    let sample_count = usize::try_from(image.width())
        .ok()
        .and_then(|width| {
            usize::try_from(image.height())
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(usize::from(P::CHANNEL_COUNT)));
    let Some(sample_count) = sample_count else {
        return &[];
    };
    image.as_raw().get(..sample_count).unwrap_or_default()
}

pub(crate) fn byte_bands_1(samples: &[u8]) -> Vec<(u8, u8)> {
    Vec::from(extrema_by_channel::<_, 1, 1>(samples, [0], |sample| sample))
}

pub(crate) fn byte_bands_2(samples: &[u8]) -> Vec<(u8, u8)> {
    Vec::from(extrema_by_channel::<_, 2, 2>(samples, [0, 1], |sample| {
        sample
    }))
}

pub(crate) fn byte_bands_3(samples: &[u8]) -> Vec<(u8, u8)> {
    Vec::from(extrema_by_channel::<_, 3, 3>(
        samples,
        [0, 1, 2],
        |sample| sample,
    ))
}

pub(crate) fn byte_bands_4(samples: &[u8]) -> Vec<(u8, u8)> {
    Vec::from(extrema_by_channel::<_, 4, 4>(
        samples,
        [0, 1, 2, 3],
        |sample| sample,
    ))
}

pub(crate) fn clipped_u16_luma(samples: &[u16]) -> Vec<(u8, u8)> {
    Vec::from(extrema_by_channel::<_, 1, 1>(samples, [0], |sample| {
        sample.min(u16::from(u8::MAX)) as u8
    }))
}

/// Preserve the old byte API's first two RGBA projection lanes for LA16.
pub(crate) fn projected_u16_luma_alpha(samples: &[u16]) -> Vec<(u8, u8)> {
    let luma = extrema_by_channel::<_, 2, 1>(samples, [0], |sample| u8::from_primitive(sample))[0];
    vec![luma, luma]
}

pub(crate) fn u16_bands_3(samples: &[u16]) -> Vec<(u8, u8)> {
    Vec::from(extrema_by_channel::<_, 3, 3>(
        samples,
        [0, 1, 2],
        |sample| u8::from_primitive(sample),
    ))
}

pub(crate) fn u16_bands_4(samples: &[u16]) -> Vec<(u8, u8)> {
    Vec::from(extrema_by_channel::<_, 4, 4>(
        samples,
        [0, 1, 2, 3],
        |sample| u8::from_primitive(sample),
    ))
}

/// Preserve the former RGBA8 fallback's opaque fourth lane for RGB32F.
pub(crate) fn f32_bands_3_with_opaque_alpha(samples: &[f32]) -> Vec<(u8, u8)> {
    let mut extrema = Vec::from(extrema_by_channel::<_, 3, 3>(
        samples,
        [0, 1, 2],
        |sample| u8::from_primitive(sample),
    ));
    extrema.push(if samples.is_empty() {
        (u8::MAX, u8::MIN)
    } else {
        (u8::MAX, u8::MAX)
    });
    extrema
}

pub(crate) fn f32_bands_4(samples: &[f32]) -> Vec<(u8, u8)> {
    Vec::from(extrema_by_channel::<_, 4, 4>(
        samples,
        [0, 1, 2, 3],
        |sample| u8::from_primitive(sample),
    ))
}

#[inline]
fn extrema_by_channel<T: Copy, const SOURCE_CHANNELS: usize, const OUTPUT_CHANNELS: usize>(
    samples: &[T],
    channel_map: [usize; OUTPUT_CHANNELS],
    mut to_byte: impl FnMut(T) -> u8,
) -> [(u8, u8); OUTPUT_CHANNELS] {
    let mut extrema = [(u8::MAX, u8::MIN); OUTPUT_CHANNELS];
    for pixel in samples.chunks_exact(SOURCE_CHANNELS) {
        for (extrema, &channel) in extrema.iter_mut().zip(&channel_map) {
            let sample = to_byte(pixel[channel]);
            extrema.0 = extrema.0.min(sample);
            extrema.1 = extrema.1.max(sample);
        }
    }
    extrema
}
