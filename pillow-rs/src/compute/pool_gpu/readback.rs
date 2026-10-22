//! GPU buffer mapping and readback decoding for image results.

use super::*;

impl GpuInner {
    pub(super) fn readback_bytes(
        &self,
        size: u64,
        staging: &wgpu::Buffer,
    ) -> Result<Vec<u8>, PilError> {
        self.readback_with(size, staging, |bytes| Ok(bytes.to_vec()))
    }

    pub(super) fn wait_for_buffer_mapping(
        &self,
        size: u64,
        buffer: &wgpu::Buffer,
        mode: wgpu::MapMode,
        stage: &str,
    ) -> Result<(), PilError> {
        let slice = buffer.slice(..size);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(mode, move |r| {
            let _ = tx.send(r);
        });
        let deadline = Instant::now() + GPU_READBACK_TIMEOUT;
        let mut empty_polls = 0usize;
        loop {
            self.poll_device(stage)?;
            match rx.try_recv() {
                Ok(Ok(())) => break,
                Ok(Err(error)) => {
                    return Err(PilError::ValueError(format!(
                        "{stage} map_async failed: {error:?}"
                    )));
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    return Err(PilError::ValueError(format!(
                        "{stage} channel closed before completion"
                    )));
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    empty_polls = empty_polls.saturating_add(1);
                    if let Some(backoff) =
                        readback_poll_backoff(true, empty_polls, Instant::now(), deadline)
                    {
                        std::thread::sleep(backoff);
                        continue;
                    }
                    let detail = self
                        .failure_detail()
                        .unwrap_or_else(|| "device did not complete the submission".into());
                    let message = format!(
                        "{stage} timed out after {}s: {detail}",
                        GPU_READBACK_TIMEOUT.as_secs()
                    );
                    self.mark_failed(message.clone());
                    // Do not leave a wedged native queue available to a later
                    // call in this process. The process-level parity runner
                    // also terminates the isolated child on its own deadline.
                    self.device.destroy();
                    return Err(PilError::ValueError(message));
                }
            }
        }
        Ok(())
    }

    /// Decode while the readback buffer is mapped, then unmap even when
    /// decoding fails. This accepts either staging or batch-owned primary
    /// storage; no later submission may use it until this function returns.
    /// The returned value cannot borrow the temporary mapping, so recycling
    /// remains safe after this function returns successfully.
    pub(super) fn readback_with<T>(
        &self,
        size: u64,
        staging: &wgpu::Buffer,
        decode: impl FnOnce(&[u8]) -> Result<T, PilError>,
    ) -> Result<T, PilError> {
        self.wait_for_buffer_mapping(size, staging, wgpu::MapMode::Read, "GPU readback")?;
        let slice = staging.slice(..size);
        let data = {
            let mapped = slice.get_mapped_range();
            decode(&mapped)
        };
        let _ = slice;
        staging.unmap();
        data
    }

    pub(super) fn readback_to_image(
        &self,
        w: u32,
        h: u32,
        staging: &wgpu::Buffer,
        native_rgb: bool,
    ) -> Result<DynamicImage, PilError> {
        let size = CheckedDims::new(w, h, 4)?.total_bytes() as u64;
        if native_rgb {
            return self
                .readback_with(size, staging, |bytes| rgb_from_packed_readback(w, h, bytes));
        }
        let data = self.readback_bytes(size, staging)?;

        let n = CheckedDims::new(w, h, 1)?.total_pixels();
        #[cfg(target_endian = "little")]
        {
            let expected = n * std::mem::size_of::<u32>();
            if data.len() != expected {
                return Err(PilError::ValueError(format!(
                    "GPU readback byte length {} does not match image size {expected}",
                    data.len()
                )));
            }
            return RgbaImage::from_raw(w, h, data)
                .map(DynamicImage::ImageRgba8)
                .ok_or_else(|| PilError::ValueError("bad readback buffer".into()));
        }

        #[cfg(target_endian = "big")]
        let mut rgba_bytes = Vec::with_capacity(n * 4);
        #[cfg(target_endian = "big")]
        let mut pixels = data.chunks_exact(std::mem::size_of::<u32>());
        #[cfg(target_endian = "big")]
        for _ in 0..n {
            let bytes = pixels.next().ok_or_else(|| {
                PilError::ValueError("GPU readback buffer ended before image pixels".into())
            })?;
            let pixel = u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            rgba_bytes.push((pixel & 0xff) as u8);
            rgba_bytes.push(((pixel >> 8) & 0xff) as u8);
            rgba_bytes.push(((pixel >> 16) & 0xff) as u8);
            rgba_bytes.push(((pixel >> 24) & 0xff) as u8);
        }
        #[cfg(target_endian = "big")]
        if !pixels.remainder().is_empty() {
            return Err(PilError::ValueError(
                "GPU readback buffer has a partial pixel".into(),
            ));
        }

        #[cfg(target_endian = "big")]
        let img = RgbaImage::from_raw(w, h, rgba_bytes)
            .ok_or_else(|| PilError::ValueError("bad readback buffer".into()))?;
        #[cfg(target_endian = "big")]
        Ok(DynamicImage::ImageRgba8(img))
    }

    #[cfg(target_endian = "little")]
    pub(super) fn readback_to_luma_alpha_from_rgba_transport(
        &self,
        w: u32,
        h: u32,
        staging: &wgpu::Buffer,
    ) -> Result<DynamicImage, PilError> {
        let size = CheckedDims::new(w, h, 4)?.total_bytes() as u64;
        self.readback_with(size, staging, |packed| la_from_rgba_transport(w, h, packed))
    }

    pub(super) fn readback_to_native_channels(
        &self,
        w: u32,
        h: u32,
        channels: u8,
        staging: &wgpu::Buffer,
    ) -> Result<DynamicImage, PilError> {
        let dimensions = CheckedDims::new(w, h, channels)?;
        let native_bytes = dimensions.total_bytes();
        let expected_transfer_bytes = native_bytes
            .div_ceil(std::mem::size_of::<u32>())
            .checked_mul(std::mem::size_of::<u32>())
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or_else(|| {
                PilError::ValueError("GPU native image readback size overflow".into())
            })?;
        self.readback_with(expected_transfer_bytes, staging, |mapped| {
            if mapped.len() != expected_transfer_bytes as usize {
                return Err(PilError::InternalError(
                    "GPU native image readback has an unexpected byte length".into(),
                ));
            }
            let pixels = mapped[..native_bytes].to_vec();
            crate::compute::record_pipeline_allocation(pixels.len());
            crate::image_utils::raw_bytes_to_image(w, h, pixels, usize::from(channels))
        })
    }

    pub(super) fn readback_to_luma8(
        &self,
        w: u32,
        h: u32,
        staging: &wgpu::Buffer,
    ) -> Result<DynamicImage, PilError> {
        let pixel_count = CheckedDims::new(w, h, 1)?.total_pixels();
        let transfer_bytes = compact_luma8_transfer_bytes(w, h)?;
        let transfer_len = usize::try_from(transfer_bytes)
            .map_err(|_| PilError::ValueError("GPU luma readback is too large".into()))?;
        self.readback_with(transfer_bytes, staging, |bytes| {
            if bytes.len() != transfer_len || bytes.len() < pixel_count {
                return Err(PilError::ValueError(format!(
                    "GPU packed-luma readback byte length {} does not match expected size {}",
                    bytes.len(),
                    transfer_len
                )));
            }
            let samples = bytes[..pixel_count].to_vec();
            crate::compute::record_pipeline_allocation(samples.len());
            crate::raster::GrayImage::from_raw(w, h, samples)
                .map(DynamicImage::ImageLuma8)
                .ok_or_else(|| PilError::ValueError("bad luma readback buffer".into()))
        })
    }

    pub(super) fn readback_to_cover_native_bytes(
        &self,
        w: u32,
        h: u32,
        dispatch: NativeCoverPackedOutputDispatch,
        staging: &wgpu::Buffer,
    ) -> Result<DynamicImage, PilError> {
        let transfer_len = usize::try_from(dispatch.transfer_bytes)
            .map_err(|_| PilError::ValueError("GPU Cover readback is too large".into()))?;
        self.readback_with(dispatch.transfer_bytes, staging, |bytes| {
            if bytes.len() != transfer_len {
                return Err(PilError::InternalError(
                    "GPU Cover packed readback length mismatch".into(),
                ));
            }
            let samples =
                unpack_native_cover_packed_rows(w, h, dispatch.channels, dispatch, bytes)?;
            crate::compute::record_pipeline_allocation(samples.len());
            crate::image_utils::raw_bytes_to_image(w, h, samples, usize::from(dispatch.channels))
        })
    }

    pub(super) fn readback_to_luma16(
        &self,
        w: u32,
        h: u32,
        staging: &wgpu::Buffer,
        mode: Option<&str>,
    ) -> Result<DynamicImage, PilError> {
        let size = CheckedDims::new(w, h, 4)?.total_bytes() as u64;
        let data = self.readback_bytes(size, staging)?;
        let n = CheckedDims::new(w, h, 1)?.total_pixels();
        let expected = n * std::mem::size_of::<u32>();
        if data.len() != expected {
            return Err(PilError::ValueError(format!(
                "GPU readback byte length {} does not match image size {expected}",
                data.len()
            )));
        }
        let mut pixels = Vec::with_capacity(n);
        for sample in data.chunks_exact(std::mem::size_of::<u32>()) {
            let bytes = [sample[0], sample[1]];
            pixels.push(if matches!(mode, Some("I;16B" | "I;16N")) {
                u16::from_be_bytes(bytes)
            } else {
                u16::from_ne_bytes(bytes)
            });
        }
        if !data
            .chunks_exact(std::mem::size_of::<u32>())
            .remainder()
            .is_empty()
        {
            return Err(PilError::ValueError(
                "GPU readback buffer has a partial typed sample".into(),
            ));
        }
        let image = ImageBuffer::<Luma<u16>, Vec<u16>>::from_raw(w, h, pixels)
            .ok_or_else(|| PilError::ValueError("bad typed readback buffer".into()))?;
        Ok(DynamicImage::ImageLuma16(image))
    }

    /// Read back the numeric-u16 representation used by typed Paste. Unlike
    /// I;16 geometry, this path deliberately ignores the public byte-order
    /// label: the shader blended decoded unsigned samples and wrote the
    /// result as a little-endian numeric word. The returned host image keeps
    /// those samples ready for the existing mode-preserving boundary.
    pub(super) fn readback_to_luma16_numeric(
        &self,
        w: u32,
        h: u32,
        staging: &wgpu::Buffer,
    ) -> Result<DynamicImage, PilError> {
        let size = CheckedDims::new(w, h, 4)?.total_bytes() as u64;
        let data = self.readback_bytes(size, staging)?;
        let n = CheckedDims::new(w, h, 1)?.total_pixels();
        let expected = n * std::mem::size_of::<u32>();
        if data.len() != expected {
            return Err(PilError::ValueError(format!(
                "GPU readback byte length {} does not match image size {expected}",
                data.len()
            )));
        }
        let pixels = data
            .chunks_exact(std::mem::size_of::<u32>())
            .map(|sample| u16::from_le_bytes([sample[0], sample[1]]))
            .collect::<Vec<_>>();
        if !data
            .chunks_exact(std::mem::size_of::<u32>())
            .remainder()
            .is_empty()
        {
            return Err(PilError::ValueError(
                "GPU readback buffer has a partial typed sample".into(),
            ));
        }
        let image = ImageBuffer::<Luma<u16>, Vec<u16>>::from_raw(w, h, pixels)
            .ok_or_else(|| PilError::ValueError("bad typed readback buffer".into()))?;
        Ok(DynamicImage::ImageLuma16(image))
    }
}
