//! Native output assembly shared by the exact-mode ConvertTransparent strategies.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::buffer::HiddenPixelLanes;
use crate::raster::{DynamicImage, Mode};

pub(crate) struct TransparentOutput {
    width: u32,
    height: u32,
    target: Mode,
    expected_pixels: usize,
    expected_bytes: usize,
    bytes: Vec<u8>,
    hidden_lanes: Option<Vec<u8>>,
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
pub(crate) fn execute_gray_samples(
    width: u32,
    height: u32,
    target: Mode,
    key: [u8; 3],
    samples: impl IntoIterator<Item = u8>,
) -> Result<DynamicImage, PilError> {
    let gray_key = [key[0]; 3];
    let mut output = TransparentOutput::new(width, height, target)?;
    for sample in samples {
        output.push_gray(sample, sample == gray_key[0], gray_key);
    }
    output.finish()
}

impl TransparentOutput {
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    pub(crate) fn new(width: u32, height: u32, target: Mode) -> Result<Self, PilError> {
        let channels = match target {
            Mode::ImageRgba8 | Mode::ImageRgbPremultiplied8 => 4,
            Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 => 2,
            _ => {
                return Err(PilError::InternalError(
                    "ConvertTransparent selected an undeclared output mode".into(),
                ));
            }
        };
        let dimensions = CheckedDims::new_allow_empty(width, height, channels)?;
        Ok(Self {
            width,
            height,
            target,
            expected_pixels: dimensions.total_pixels(),
            expected_bytes: dimensions.total_bytes(),
            bytes: Vec::with_capacity(dimensions.total_bytes()),
            hidden_lanes: None,
        })
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    pub(crate) fn push_gray(&mut self, sample: u8, transparent: bool, key: [u8; 3]) {
        self.push_rgb([sample; 3], sample, transparent, key);
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    pub(crate) fn push_rgb(&mut self, rgb: [u8; 3], luma: u8, transparent: bool, key: [u8; 3]) {
        match self.target {
            Mode::ImageRgba8 => self.bytes.extend_from_slice(&[
                rgb[0],
                rgb[1],
                rgb[2],
                if transparent { 0 } else { u8::MAX },
            ]),
            Mode::ImageRgbPremultiplied8 if transparent => self.bytes.extend_from_slice(&[0; 4]),
            Mode::ImageRgbPremultiplied8 => {
                self.bytes
                    .extend_from_slice(&[rgb[0], rgb[1], rgb[2], u8::MAX]);
            }
            Mode::ImageLumaA8 if transparent => self.push_luma_alpha(key[0], 0, [key[1], key[2]]),
            Mode::ImageLumaA8 => self.push_luma_alpha(luma, u8::MAX, [luma, luma]),
            Mode::ImageLaPremultiplied8 if transparent => self.push_luma_alpha(0, 0, [0, 0]),
            Mode::ImageLaPremultiplied8 => {
                self.push_luma_alpha(luma, u8::MAX, [luma, luma]);
            }
            _ => unreachable!("output mode was validated in TransparentOutput::new"),
        }
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    pub(crate) fn finish(self) -> Result<DynamicImage, PilError> {
        if self.bytes.len() != self.expected_bytes {
            return Err(PilError::InternalError(
                "ConvertTransparent output shape mismatch".into(),
            ));
        }
        if !self.bytes.is_empty() {
            crate::compute::record_pipeline_allocation(self.bytes.capacity());
        }
        let mut output = crate::compute::pool_cpu::helpers::dynamic_image::from_raw_bytes(
            self.target,
            self.width,
            self.height,
            self.bytes,
        )
        .ok_or_else(|| {
            PilError::InternalError("ConvertTransparent output allocation failed".into())
        })?;
        if let Some(lanes) = self.hidden_lanes {
            if !lanes.is_empty() {
                crate::compute::record_pipeline_allocation(lanes.capacity());
            }
            let lanes =
                HiddenPixelLanes::per_pixel(2, self.expected_pixels, lanes).ok_or_else(|| {
                    PilError::InternalError("ConvertTransparent hidden-lane extent mismatch".into())
                })?;
            if !output.replace_hidden_pixel_lanes(Some(lanes)) {
                return Err(PilError::InternalError(
                    "ConvertTransparent output rejected its hidden lanes".into(),
                ));
            }
        }
        Ok(output)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn push_luma_alpha(&mut self, luma: u8, alpha: u8, hidden: [u8; 2]) {
        if self.hidden_lanes.is_none() && hidden != [luma, luma] {
            let mut lanes = Vec::with_capacity(self.expected_pixels.saturating_mul(2));
            for pixel in self.bytes.chunks_exact(2) {
                lanes.extend_from_slice(&[pixel[0], pixel[0]]);
            }
            self.hidden_lanes = Some(lanes);
        }
        self.bytes.extend_from_slice(&[luma, alpha]);
        if let Some(lanes) = &mut self.hidden_lanes {
            lanes.extend_from_slice(&hidden);
        }
    }
}
