//! GPU buffer mapping and readback decoding for image results.

use super::*;

/// `Maintain::Wait` can wait forever when a native device or driver wedges.
/// Poll with a short bounded backoff so the library remains responsive while
/// retaining a finite failure path for a wedged native device or driver.
pub(super) const GPU_READBACK_TIMEOUT: Duration = Duration::from_secs(30);
/// Even large transfers can finish before the native scheduler's 1 ms timer
/// granularity. Give every read a bounded initial polling window; long jobs
/// return to the conservative backoff after a fixed retry count. Sixty-four
/// retries avoid ending this window just before a short compute frame
/// completes and paying a whole additional millisecond of scheduler delay.
pub(super) const GPU_POLL_FAST_BACKOFF: Duration = Duration::from_micros(50);
pub(super) const GPU_POLL_FAST_RETRIES: usize = 64;
pub(super) const GPU_POLL_BACKOFF: Duration = Duration::from_millis(1);

pub(super) fn readback_poll_backoff(
    fast_polling: bool,
    empty_polls: usize,
    now: Instant,
    deadline: Instant,
) -> Option<Duration> {
    let remaining = deadline.saturating_duration_since(now);
    (!remaining.is_zero()).then(|| {
        let backoff = if fast_polling && empty_polls <= GPU_POLL_FAST_RETRIES {
            GPU_POLL_FAST_BACKOFF
        } else {
            GPU_POLL_BACKOFF
        };
        backoff.min(remaining)
    })
}

pub(super) fn unpack_native_cover_packed_rows(
    width: u32,
    height: u32,
    channels: u8,
    dispatch: NativeCoverPackedOutputDispatch,
    packed: &[u8],
) -> Result<Vec<u8>, PilError> {
    let pixel_count = CheckedDims::new(width, height, channels)?.total_bytes();
    let row_words = usize::try_from(dispatch.words_per_row)
        .map_err(|_| PilError::ValueError("GPU Cover output row is too large".into()))?;
    let row_bytes = row_words
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| PilError::ValueError("GPU Cover output row is too large".into()))?;
    let expected_words = row_words
        .checked_mul(
            usize::try_from(height)
                .map_err(|_| PilError::ValueError("GPU Cover output height is too large".into()))?,
        )
        .ok_or_else(|| PilError::ValueError("GPU Cover output is too large".into()))?;
    let expected_transfer = expected_words
        .checked_mul(std::mem::size_of::<u32>())
        .ok_or_else(|| PilError::ValueError("GPU Cover output is too large".into()))?;
    let native_row_bytes = usize::try_from(width)
        .ok()
        .and_then(|width| width.checked_mul(usize::from(channels)))
        .ok_or_else(|| PilError::ValueError("GPU Cover output row is too large".into()))?;
    if dispatch.channels != channels
        || expected_words != dispatch.word_count as usize
        || expected_transfer as u64 != dispatch.transfer_bytes
        || packed.len() != expected_transfer
        || row_bytes < native_row_bytes
    {
        return Err(PilError::InternalError(
            "GPU packed Cover readback does not match its row layout".into(),
        ));
    }
    let mut samples = Vec::with_capacity(pixel_count);
    for row in 0..height as usize {
        let row_start = row
            .checked_mul(row_bytes)
            .ok_or_else(|| PilError::ValueError("GPU Cover output offset overflow".into()))?;
        let row_end = row_start
            .checked_add(native_row_bytes)
            .ok_or_else(|| PilError::ValueError("GPU Cover output offset overflow".into()))?;
        let row_pixels = packed.get(row_start..row_end).ok_or_else(|| {
            PilError::InternalError("GPU packed Cover readback ended mid-row".into())
        })?;
        samples.extend_from_slice(row_pixels);
    }
    Ok(samples)
}

/// Extract native RGB bytes directly from the mapped packed-word transport.
/// Alpha is discarded exactly as RGBA-to-RGB mode preservation does; no
/// channel arithmetic or logical color conversion belongs at this boundary.
pub(super) fn rgb_from_packed_readback(
    w: u32,
    h: u32,
    packed: &[u8],
) -> Result<DynamicImage, PilError> {
    let packed_dims = CheckedDims::new(w, h, 4)?;
    if packed.len() != packed_dims.total_bytes() {
        return Err(PilError::ValueError(format!(
            "GPU readback byte length {} does not match image size {}",
            packed.len(),
            packed_dims.total_bytes()
        )));
    }
    let rgb_dims = CheckedDims::new(w, h, 3)?;
    let mut rgb = rgb_dims.alloc_buffer();
    let unpack_row = |row: &mut [u8], y: usize| {
        let start = y * packed_dims.row_stride();
        let source = &packed[start..start + packed_dims.row_stride()];
        for (source, destination) in source.chunks_exact(4).zip(row.chunks_exact_mut(3)) {
            #[cfg(target_endian = "little")]
            destination.copy_from_slice(&source[..3]);
            #[cfg(target_endian = "big")]
            {
                // Match readback_to_image's native-word decoding before
                // removing alpha on hosts whose byte order differs.
                let pixel = u32::from_ne_bytes([source[0], source[1], source[2], source[3]]);
                destination.copy_from_slice(&[
                    (pixel & 0xff) as u8,
                    ((pixel >> 8) & 0xff) as u8,
                    ((pixel >> 16) & 0xff) as u8,
                ]);
            }
        }
    };
    // Keep GPU result materialization independent of the Parallel CPU
    // feature; the GPU path must not schedule host Rayon work.
    for (y, row) in rgb.chunks_exact_mut(rgb_dims.row_stride()).enumerate() {
        unpack_row(row, y);
    }
    crate::raster::RgbImage::from_raw(w, h, rgb)
        .map(DynamicImage::ImageRgb8)
        .ok_or_else(|| PilError::InternalError("GPU RGB readback shape mismatch".into()))
}

/// Narrow packed RGBA transport to LA without first constructing an RGBA
/// image. Native-LA Sharpness emits its filtered luma in R/G/B and preserves
/// alpha in A, matching the generic transport's mode-preserving boundary.
#[cfg(target_endian = "little")]
pub(super) fn la_from_rgba_transport(
    w: u32,
    h: u32,
    packed: &[u8],
) -> Result<DynamicImage, PilError> {
    let packed_dims = CheckedDims::new(w, h, 4)?;
    if packed.len() != packed_dims.total_bytes() {
        return Err(PilError::ValueError(format!(
            "GPU readback byte length {} does not match image size {}",
            packed.len(),
            packed_dims.total_bytes()
        )));
    }
    let la_dims = CheckedDims::new(w, h, 2)?;
    let mut la = la_dims.alloc_buffer();
    for (source, destination) in packed.chunks_exact(4).zip(la.chunks_exact_mut(2)) {
        destination.copy_from_slice(&[source[0], source[3]]);
    }
    crate::raster::GrayAlphaImage::from_raw(w, h, la)
        .map(DynamicImage::ImageLumaA8)
        .ok_or_else(|| PilError::InternalError("GPU LA readback shape mismatch".into()))
}

impl GpuInner {
    pub(super) fn prepare_readback(
        &self,
        source: &wgpu::Buffer,
        size: u64,
    ) -> Result<ReadbackTarget, PilError> {
        // Use the resource's actual usage so a staged working set remains
        // staged even when this device supports directly mapped storage.
        if source.usage().contains(wgpu::BufferUsages::MAP_READ) {
            Ok(ReadbackTarget::Primary)
        } else {
            self.acquire_staging(size).map(ReadbackTarget::Staging)
        }
    }

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
