//! Specialized native-layout routes used before the general batch upload.
//!
//! Each route acquires, executes with, and recycles its own working buffers.
//! Unmatched operations continue through the ordinary packed batch path.

use super::*;

impl GpuInner {
    #[cfg(target_endian = "little")]
    pub(super) fn try_execute_native_batch_routes(
        &self,
        ops: &[PipelineOp],
        image: &DynamicImage,
        mode: Option<&str>,
        auxiliary_images: &[AuxiliaryImages],
        limits: &wgpu::Limits,
    ) -> Result<Option<DynamicImage>, PilError> {
        if let Some(result) =
            self.try_execute_native_paste_route(ops, image, mode, auxiliary_images, limits)?
        {
            return Ok(Some(result));
        }
        if let Some(result) =
            self.try_execute_native_composite_route(ops, image, mode, auxiliary_images)?
        {
            return Ok(Some(result));
        }
        if let Some(result) = self.try_execute_native_put_alpha_data_route(
            ops,
            image,
            mode,
            auxiliary_images,
            limits,
        )? {
            return Ok(Some(result));
        }
        if let Some(result) = self.try_execute_native_put_alpha_route(ops, image, mode, limits)? {
            return Ok(Some(result));
        }
        self.try_execute_native_byte_op_route(ops, image, mode, auxiliary_images)
    }

    #[cfg(target_endian = "little")]
    fn try_execute_native_paste_route(
        &self,
        ops: &[PipelineOp],
        image: &DynamicImage,
        mode: Option<&str>,
        auxiliary_images: &[AuxiliaryImages],
        limits: &wgpu::Limits,
    ) -> Result<Option<DynamicImage>, PilError> {
        if let ([op], [auxiliary]) = (ops, auxiliary_images)
            && let Some(source_pixels) = auxiliary.second.as_deref()
            && let Some(offset) =
                gpu_native_rgb_to_rgba_paste_layout(op, image, source_pixels, mode)
            && let Some(dispatch) = plan_gpu_native_rgb_to_rgba_paste(
                image.width(),
                image.height(),
                source_pixels.width(),
                source_pixels.height(),
                limits.max_compute_workgroups_per_dimension,
                limits.max_storage_buffer_binding_size,
                limits.max_buffer_size,
            )
        {
            let mut buffers = self.acquire_buffers(dispatch.destination_pixels)?;
            let result = self.execute_native_rgb_to_rgba_paste(
                image,
                source_pixels,
                offset,
                dispatch,
                &mut buffers,
            )?;
            self.recycle_buffers(buffers);
            return Ok(Some(result));
        }

        if let ([op], [auxiliary]) = (ops, auxiliary_images)
            && let Some(source_pixels) = auxiliary.second.as_deref()
            && let Some((offset, bytes_per_pixel)) =
                gpu_native_byte_paste_layout(op, image, source_pixels, mode)
            && let Some(dispatch) = plan_gpu_native_byte_paste(
                image.width(),
                image.height(),
                source_pixels.width(),
                source_pixels.height(),
                bytes_per_pixel,
                limits.max_compute_workgroups_per_dimension,
                limits.max_storage_buffer_binding_size,
                limits.max_buffer_size,
            )
        {
            let mut buffers = self.acquire_buffers(dispatch.word_count)?;
            let result = self.execute_native_byte_paste(
                image,
                source_pixels,
                offset,
                dispatch,
                &mut buffers,
            )?;
            self.recycle_buffers(buffers);
            return Ok(Some(result));
        }
        Ok(None)
    }

    #[cfg(target_endian = "little")]
    fn try_execute_native_composite_route(
        &self,
        ops: &[PipelineOp],
        image: &DynamicImage,
        mode: Option<&str>,
        auxiliary_images: &[AuxiliaryImages],
    ) -> Result<Option<DynamicImage>, PilError> {
        if let (
            [
                PipelineOp::CompositeModule {
                    other,
                    mask,
                    mask_alpha,
                },
            ],
            [auxiliary],
        ) = (ops, auxiliary_images)
            && let (Some(other_image), Some(mask_image)) =
                (auxiliary.second.as_deref(), auxiliary.third.as_deref())
            && let Some((channels, mask_channels, mask_channel)) = gpu_native_composite_layout(
                image,
                mode,
                other,
                other_image,
                mask,
                mask_image,
                *mask_alpha,
            )
        {
            let bytes = CheckedDims::new(image.width(), image.height(), channels)?.total_bytes();
            let words = u32::try_from(bytes.div_ceil(4))
                .map_err(|_| PilError::ValueError("GPU native composite is too large".into()))?;
            let mut buffers = self.acquire_buffers(words)?;
            let result = self.execute_native_composite(
                image,
                other_image,
                mask_image,
                channels,
                mask_channels,
                mask_channel,
                &mut buffers,
            )?;
            self.recycle_buffers(buffers);
            return Ok(Some(result));
        }
        Ok(None)
    }

    #[cfg(target_endian = "little")]
    fn try_execute_native_put_alpha_data_route(
        &self,
        ops: &[PipelineOp],
        image: &DynamicImage,
        mode: Option<&str>,
        auxiliary_images: &[AuxiliaryImages],
        limits: &wgpu::Limits,
    ) -> Result<Option<DynamicImage>, PilError> {
        if let Some(result) = self.try_execute_native_rgb_put_alpha_data_route(
            ops,
            image,
            mode,
            auxiliary_images,
            limits,
        )? {
            return Ok(Some(result));
        }
        self.try_execute_native_rgba_put_alpha_data_route(
            ops,
            image,
            mode,
            auxiliary_images,
            limits,
        )
    }

    #[cfg(target_endian = "little")]
    fn try_execute_native_rgb_put_alpha_data_route(
        &self,
        ops: &[PipelineOp],
        image: &DynamicImage,
        mode: Option<&str>,
        auxiliary_images: &[AuxiliaryImages],
        limits: &wgpu::Limits,
    ) -> Result<Option<DynamicImage>, PilError> {
        if let (
            [
                PipelineOp::PutAlphaData {
                    mode: PixelMode::RGB,
                    ..
                },
            ],
            [auxiliary],
        ) = (ops, auxiliary_images)
            && matches!(image, DynamicImage::ImageRgb8(_))
            && matches!(mode, None | Some("RGB"))
            && let Some(alpha_mask) = auxiliary.third.as_deref()
            && matches!(alpha_mask, DynamicImage::ImageLuma8(_))
            && alpha_mask.dimensions() == image.dimensions()
            && let Some(dispatch) = plan_native_rgb_put_alpha_data_dispatch(
                image.width(),
                image.height(),
                limits.max_compute_workgroups_per_dimension,
                limits.max_storage_buffer_binding_size,
                limits.max_buffer_size,
            )
        {
            let pixels = CheckedDims::new(image.width(), image.height(), 1)?.total_pixels();
            let capacity = u32::try_from(pixels).map_err(|_| {
                PilError::ValueError("GPU native RGB PutAlphaData image is too large".into())
            })?;
            let mut buffers = self.acquire_buffers(capacity)?;
            let result =
                self.execute_native_rgb_put_alpha_data(image, alpha_mask, dispatch, &mut buffers)?;
            self.recycle_buffers(buffers);
            return Ok(Some(result));
        }
        Ok(None)
    }

    #[cfg(target_endian = "little")]
    fn try_execute_native_rgba_put_alpha_data_route(
        &self,
        ops: &[PipelineOp],
        image: &DynamicImage,
        mode: Option<&str>,
        auxiliary_images: &[AuxiliaryImages],
        limits: &wgpu::Limits,
    ) -> Result<Option<DynamicImage>, PilError> {
        if let (
            [
                PipelineOp::PutAlphaData {
                    mode: PixelMode::RGBA,
                    ..
                },
            ],
            [auxiliary],
        ) = (ops, auxiliary_images)
            && matches!(image, DynamicImage::ImageRgba8(_))
            && matches!(mode, None | Some("RGBA"))
            && let Some(alpha_mask) = auxiliary.third.as_deref()
            && matches!(alpha_mask, DynamicImage::ImageLuma8(_))
            && alpha_mask.dimensions() == image.dimensions()
            && let Some(dispatch) = plan_native_rgba_put_alpha_data_dispatch(
                image.width(),
                image.height(),
                limits.max_compute_workgroups_per_dimension,
                limits.max_storage_buffer_binding_size,
                limits.max_buffer_size,
            )
        {
            let pixels = CheckedDims::new(image.width(), image.height(), 1)?.total_pixels();
            let capacity = u32::try_from(pixels).map_err(|_| {
                PilError::ValueError("GPU native RGBA PutAlphaData image is too large".into())
            })?;
            let mut buffers = self.acquire_buffers(capacity)?;
            let result =
                self.execute_native_rgba_put_alpha_data(image, alpha_mask, dispatch, &mut buffers)?;
            self.recycle_buffers(buffers);
            return Ok(Some(result));
        }
        Ok(None)
    }

    #[cfg(target_endian = "little")]
    fn try_execute_native_put_alpha_route(
        &self,
        ops: &[PipelineOp],
        image: &DynamicImage,
        mode: Option<&str>,
        limits: &wgpu::Limits,
    ) -> Result<Option<DynamicImage>, PilError> {
        if let [
            PipelineOp::PutAlpha {
                alpha,
                mode: PixelMode::RGB,
            },
        ] = ops
            && matches!(image, DynamicImage::ImageRgb8(_))
            && matches!(mode, None | Some("RGB"))
            && let Some(dispatch) = plan_native_rgb_put_alpha_dispatch(
                image.width(),
                image.height(),
                limits.max_compute_workgroups_per_dimension,
                limits.max_storage_buffer_binding_size,
                limits.max_buffer_size,
            )
        {
            let pixels = CheckedDims::new(image.width(), image.height(), 1)?.total_pixels();
            let capacity = u32::try_from(pixels).map_err(|_| {
                PilError::ValueError("GPU native RGB PutAlpha image is too large".into())
            })?;
            let mut buffers = self.acquire_buffers(capacity)?;
            let result =
                self.execute_native_rgb_put_alpha(image, *alpha, dispatch, &mut buffers)?;
            self.recycle_buffers(buffers);
            return Ok(Some(result));
        }
        Ok(None)
    }

    #[cfg(target_endian = "little")]
    fn try_execute_native_byte_op_route(
        &self,
        ops: &[PipelineOp],
        image: &DynamicImage,
        mode: Option<&str>,
        auxiliary_images: &[AuxiliaryImages],
    ) -> Result<Option<DynamicImage>, PilError> {
        let native_byte_op = match (ops, auxiliary_images) {
            ([op], [auxiliary]) => Some((op, auxiliary, false)),
            ([op, _], [auxiliary, _]) if can_fuse_gpu_multiply_screen(ops, 0) => {
                Some((op, auxiliary, true))
            }
            _ => None,
        };
        if let Some((op, auxiliary, fused_screen)) = native_byte_op
            && let Some(channels) = gpu_native_byte_op_channels(op, image, mode)
            && auxiliary.second.as_deref().is_none_or(|other| {
                image.color() == other.color() && image.dimensions() == other.dimensions()
            })
        {
            // Public/adapter guards have run. Keep native sample transport for
            // one admitted operation or the exact shared-secondary blend pair.
            let bytes = CheckedDims::new(image.width(), image.height(), channels)?.total_bytes();
            let words = u32::try_from(bytes.div_ceil(4)).map_err(|_| {
                PilError::ValueError("GPU native binary operation is too large".into())
            })?;
            let mut buffers = self.acquire_buffers(words)?;
            let result = self.execute_native_byte_op(
                op,
                image,
                auxiliary.second.as_deref(),
                channels,
                mode,
                fused_screen,
                &mut buffers,
            )?;
            self.recycle_buffers(buffers);
            return Ok(Some(result));
        }
        Ok(None)
    }
}
