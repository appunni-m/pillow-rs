
use crate::raster::GenericImageView;
use super::super::DrawLayout;
use crate::error::PilError;
use crate::raster::buffer::HiddenPixelLanes;
use crate::raster::{DynamicImage, Mode};

pub(super) struct SimdDrawSpanWriter {
    output: Vec<u8>,
    hidden_lanes: Option<HiddenPixelLanes>,
    pub(super) image_width: u32,
    pub(super) image_height: u32,
    mode: Mode,
    channels: usize,
    alpha_blend_rgb: bool,
    pub(super) vector_blocks: u64,
    pub(super) scalar_tail: u64,
}

impl SimdDrawSpanWriter {
    pub(super) fn new(
        image: &DynamicImage,
        layout: DrawLayout,
        alpha_blend_rgb: bool,
    ) -> Self {
        let (image_width, image_height) = image.dimensions();
        Self {
            output: image.as_bytes().to_vec(),
            hidden_lanes: image.hidden_pixel_lanes().cloned(),
            image_width,
            image_height,
            mode: layout.mode,
            channels: layout.channels,
            alpha_blend_rgb,
            vector_blocks: 0,
            scalar_tail: 0,
        }
    }

    pub(super) fn write(
        &mut self,
        x0: i32,
        y: i32,
        x1: i32,
        color: (u8, u8, u8, u8),
    ) -> Result<(), PilError> {
        self.write_span(i64::from(x0), i64::from(x1), i64::from(y), color)
    }

    pub(super) fn write_span(
        &mut self,
        x0: i64,
        x1: i64,
        y: i64,
        color: (u8, u8, u8, u8),
    ) -> Result<(), PilError> {
        let (vectors, tail) = super::spans::simd_draw_span(
            &mut self.output,
            self.image_width,
            self.image_height,
            x0,
            x1,
            y,
            self.channels,
            color,
            self.alpha_blend_rgb,
        )?;
        self.update_hidden_lanes(x0, x1, y, color)?;
        self.vector_blocks = self.vector_blocks.saturating_add(vectors);
        self.scalar_tail = self.scalar_tail.saturating_add(tail);
        Ok(())
    }

    pub(super) fn into_image(self) -> Result<DynamicImage, PilError> {
        let mut image = crate::compute::pool_cpu::helpers::dynamic_image::from_raw_bytes(
            self.mode,
            self.image_width,
            self.image_height,
            self.output,
        )
        .ok_or_else(|| PilError::InternalError("SIMD Draw output dimensions were invalid".into()))?;
        if !image.replace_hidden_pixel_lanes(self.hidden_lanes) {
            return Err(PilError::InternalError(
                "SIMD Draw hidden pixel lane extent did not match its image".into(),
            ));
        }
        Ok(image)
    }

    fn update_hidden_lanes(
        &mut self,
        x0: i64,
        x1: i64,
        y: i64,
        color: (u8, u8, u8, u8),
    ) -> Result<(), PilError> {
        let lane_count = self.mode.hidden_pixel_lane_count().unwrap_or(0);
        if lane_count == 0 || (self.channels == 3 && self.alpha_blend_rgb) {
            return Ok(());
        }
        if y < 0
            || y >= i64::from(self.image_height)
            || self.image_width == 0
            || x1 < 0
            || x0 >= i64::from(self.image_width)
        {
            return Ok(());
        }
        let start = x0.max(0) as usize;
        let end = x1.min(i64::from(self.image_width) - 1) as usize;
        if start > end {
            return Ok(());
        }

        let (values, default) = match lane_count {
            1 => ([color.3, 0], [u8::MAX, 0]),
            2 => ([color.1, color.2], [color.0, color.0]),
            _ => {
                return Err(PilError::InternalError(
                    "SIMD Draw hidden pixel lane count was invalid".into(),
                ));
            }
        };
        let row_start = usize::try_from(y)
            .ok()
            .and_then(|row| row.checked_mul(self.image_width as usize))
            .ok_or_else(|| PilError::InternalError("SIMD Draw row index overflow".into()))?;

        for x in start..=end {
            let pixel_index = row_start
                .checked_add(x)
                .ok_or_else(|| PilError::InternalError("SIMD Draw pixel index overflow".into()))?;
            if self.hidden_lanes.is_none() && values[..lane_count] == default[..lane_count] {
                continue;
            }
            if self.hidden_lanes.is_none() {
                self.hidden_lanes = Some(self.default_hidden_lanes(lane_count)?);
            }
            let Some(lanes) = self.hidden_lanes.as_mut() else {
                return Err(PilError::InternalError(
                    "SIMD Draw hidden pixel lanes were not initialized".into(),
                ));
            };
            if lanes.lane_count() != lane_count || lanes.set_pixel(pixel_index, values).is_none() {
                return Err(PilError::InternalError(
                    "SIMD Draw hidden pixel lane payload did not match its image".into(),
                ));
            }
        }
        Ok(())
    }

    fn default_hidden_lanes(&self, lane_count: usize) -> Result<HiddenPixelLanes, PilError> {
        let pixel_count = usize::try_from(self.image_width)
            .ok()
            .and_then(|width| {
                usize::try_from(self.image_height)
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .ok_or_else(|| PilError::DimensionError("SIMD Draw image extent overflow".into()))?;
        let lanes = match lane_count {
            1 => HiddenPixelLanes::constant(1, pixel_count, [u8::MAX, 0]),
            2 if self.channels == 2 => {
                let expected = pixel_count.checked_mul(2).ok_or_else(|| {
                    PilError::DimensionError("SIMD Draw hidden lane extent overflow".into())
                })?;
                if self.output.len() != expected {
                    return Err(PilError::InternalError(
                        "SIMD Draw compact image byte extent did not match its dimensions".into(),
                    ));
                }
                let mut values = Vec::with_capacity(expected);
                for pixel in self.output.chunks_exact(2) {
                    values.extend_from_slice(&[pixel[0], pixel[0]]);
                }
                HiddenPixelLanes::per_pixel(2, pixel_count, values)
            }
            _ => None,
        };
        lanes.ok_or_else(|| {
            PilError::InternalError("SIMD Draw hidden pixel lane storage was invalid".into())
        })
    }
}
