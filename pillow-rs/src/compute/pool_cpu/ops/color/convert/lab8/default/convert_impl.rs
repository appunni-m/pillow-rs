//! Serial Convert paths for native three-byte LAB source samples.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::pipeline::DitherMethod;
use crate::raster::{DynamicImage, GrayAlphaImage, GrayImage, Mode, RgbImage, RgbaImage};

pub(crate) fn execute(source: &RgbImage, target: Mode) -> Result<DynamicImage, PilError> {
    // NO_PILLOW_SUPPORT: non-identity targets outside the RGB family are Rust
    // extensions. Each path converts LAB samples directly into its target.
    match target {
        Mode::ImageLuma8 => gray_output(source).map(DynamicImage::ImageLuma8),
        Mode::ImageLumaA8 => luma_alpha_output(source).map(DynamicImage::ImageLumaA8),
        Mode::ImageLaPremultiplied8 | Mode::ImageRgbPremultiplied8 => {
            super::super::super::helpers::premultiplied::from_rgb_samples(
                source.width(),
                source.height(),
                super::super::helpers::to_srgb::srgb_pixels(source),
                target,
            )
        }
        Mode::ImageRgb8 => super::super::helpers::to_srgb::convert_to_rgb(source)
            .map(DynamicImage::ImageRgb8),
        Mode::ImageRgba8 => super::super::helpers::to_srgb::convert_to_four_byte(source)
            .map(DynamicImage::ImageRgba8),
        Mode::ImageRgbx8 => super::super::helpers::to_srgb::convert_to_four_byte(source)
            .map(DynamicImage::ImageRgbx8),
        Mode::ImageLab8 => Ok(DynamicImage::ImageLab8(source.clone())),
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
            super::super::super::helpers::luma16::from_luma8_samples(
                source.width(),
                source.height(),
                target,
                luma_samples(source),
            )
        }
        Mode::ImageCmyk8 => cmyk_output(source).map(DynamicImage::ImageCmyk8),
        Mode::ImageYCbCr8 => Ok(DynamicImage::ImageYCbCr8(
            crate::color::rgb_pixels_to_ycbcr(
                source.width(),
                source.height(),
                super::super::helpers::to_srgb::srgb_pixels(source),
            ),
        )),
        Mode::ImageHsv8 => Ok(DynamicImage::ImageHsv8(crate::color::rgb_pixels_to_hsv(
            source.width(),
            source.height(),
            super::super::helpers::to_srgb::srgb_pixels(source),
        ))),
        Mode::ImageI32 => scalar_output(source, true).map(DynamicImage::ImageI32),
        Mode::ImageF32 => scalar_output(source, false).map(DynamicImage::ImageF32),
        _ => Err(PilError::ValueError("image has wrong mode".into())),
    }
}

pub(crate) fn execute_binary(
    source: &RgbImage,
    dither: Option<&DitherMethod>,
) -> Result<DynamicImage, PilError> {
    let mut output = GrayImage::new(source.width(), source.height());
    if matches!(dither, Some(DitherMethod::None)) {
        for (destination, [red, green, blue]) in output
            .pixels_mut()
            .zip(super::super::helpers::to_srgb::srgb_pixels(source))
        {
            let gray = crate::color::grayscale_rgb_pixel(&[red, green, blue], 0);
            destination[0] = if gray >= 128 { u8::MAX } else { 0 };
        }
    } else {
        crate::compute::pool_cpu::ops::color::convert::helpers::binary::diffuse_rgb_samples(
            super::super::helpers::to_srgb::srgb_pixels(source),
            &mut output,
        )?;
    }
    Ok(DynamicImage::One(output))
}

pub(crate) fn execute_palette_target(
    source: &RgbImage,
    target: Mode,
    dither: Option<DitherMethod>,
) -> Result<super::super::super::ops::PreparedConvertOutput, PilError> {
    if !matches!(target, Mode::ImagePalette8 | Mode::ImagePaletteAlpha8) {
        return Err(PilError::ValueError("image has wrong mode".into()));
    }
    let metadata = if target == Mode::ImagePaletteAlpha8 {
        super::super::super::ops::ConvertWrapperMetadata::ExifOnly
    } else {
        super::super::super::ops::ConvertWrapperMetadata::PreserveSource
    };
    crate::compute::pool_cpu::ops::color::convert::helpers::palette::web_palette_from_pixels(
        source.width(),
        source.height(),
        super::super::helpers::to_srgb::srgb_pixels(source),
        !matches!(dither, Some(DitherMethod::None)),
        target,
        metadata,
    )
}

fn gray_output(source: &RgbImage) -> Result<GrayImage, PilError> {
    let bytes = encode_samples::<1>(source, |[red, green, blue]| {
        [crate::color::rgb_to_luma_u8(red, green, blue)]
    })?;
    GrayImage::from_raw(source.width(), source.height(), bytes)
        .ok_or_else(|| PilError::InternalError("LAB to L output shape mismatch".into()))
}

fn luma_alpha_output(source: &RgbImage) -> Result<GrayAlphaImage, PilError> {
    let bytes = encode_samples::<2>(source, |[red, green, blue]| {
        [crate::color::rgb_to_luma_u8(red, green, blue), u8::MAX]
    })?;
    GrayAlphaImage::from_raw(source.width(), source.height(), bytes)
        .ok_or_else(|| PilError::InternalError("LAB to LA output shape mismatch".into()))
}

fn cmyk_output(source: &RgbImage) -> Result<RgbaImage, PilError> {
    let bytes = encode_samples::<4>(source, |[red, green, blue]| {
        [u8::MAX - red, u8::MAX - green, u8::MAX - blue, 0]
    })?;
    RgbaImage::from_raw(source.width(), source.height(), bytes)
        .ok_or_else(|| PilError::InternalError("LAB to CMYK output shape mismatch".into()))
}

fn scalar_output(source: &RgbImage, integer: bool) -> Result<RgbaImage, PilError> {
    let bytes = if integer {
        encode_samples::<4>(source, |[red, green, blue]| {
            crate::compute::pool_cpu::ops::color::convert::helpers::pixel::scalar_i32_rgb(
                red, green, blue,
            )
        })?
    } else {
        encode_samples::<4>(source, |[red, green, blue]| {
            crate::compute::pool_cpu::ops::color::convert::helpers::pixel::scalar_f32_rgb(
                red, green, blue,
            )
        })?
    };
    RgbaImage::from_raw(source.width(), source.height(), bytes)
        .ok_or_else(|| PilError::InternalError("LAB scalar output shape mismatch".into()))
}

fn luma_samples(source: &RgbImage) -> impl Iterator<Item = u8> + '_ {
    super::super::helpers::to_srgb::srgb_pixels(source)
        .map(|[red, green, blue]| crate::color::rgb_to_luma_u8(red, green, blue))
}

fn encode_samples<const CHANNELS: usize>(
    source: &RgbImage,
    mut encode: impl FnMut([u8; 3]) -> [u8; CHANNELS],
) -> Result<Vec<u8>, PilError> {
    let dimensions = CheckedDims::new_allow_empty(source.width(), source.height(), CHANNELS)?;
    let mut output = Vec::with_capacity(dimensions.total_bytes());
    for sample in super::super::helpers::to_srgb::srgb_pixels(source) {
        output.extend_from_slice(&encode(sample));
    }
    if output.len() != dimensions.total_bytes() {
        return Err(PilError::InternalError(
            "LAB Convert output shape mismatch".into(),
        ));
    }
    if output.capacity() != 0 {
        crate::compute::record_pipeline_allocation(output.capacity());
    }
    Ok(output)
}
