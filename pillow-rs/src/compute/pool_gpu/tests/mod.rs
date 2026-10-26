use super::*;
use crate::ops::imageops::ImageOpsColor;
use crate::ops::rotate::{RotateExpandInput, RotatePointInput, RotateResampleInput};
use crate::ops::transform::{TransformData, TransformFill};
use crate::pipeline::{ColorMode, PipelineOp, PixelMode, ResampleFilter, TransformMethod};
use crate::raster::{
    DynamicImage, GenericImageView, GrayAlphaImage, GrayImage, ImageBuffer, Luma, RgbImage,
    RgbaImage,
};
use crate::{Backend, Image, ResampleInput};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(target_endian = "little")]
fn rgba_transpose_reference(
    image: &RgbaImage,
    method: &crate::pipeline::TransposeMethod,
) -> RgbaImage {
    use crate::pipeline::TransposeMethod as Method;
    let (width, height) = image.dimensions();
    let (out_width, out_height) = match method {
        Method::Rotate90 | Method::Rotate270 | Method::Transpose | Method::Transverse => {
            (height, width)
        }
        _ => (width, height),
    };
    let mut bytes = vec![0; image.as_raw().len()];
    for y in 0..height {
        for x in 0..width {
            let (target_x, target_y) = match method {
                Method::FlipLeftRight => (width - 1 - x, y),
                Method::FlipTopBottom => (x, height - 1 - y),
                Method::Rotate90 => (y, width - 1 - x),
                Method::Rotate180 => (width - 1 - x, height - 1 - y),
                Method::Rotate270 => (height - 1 - y, x),
                Method::Transpose => (y, x),
                Method::Transverse => (height - 1 - y, width - 1 - x),
            };
            let from = ((y * width + x) * 4) as usize;
            let to = ((target_y * out_width + target_x) * 4) as usize;
            bytes[to..to + 4].copy_from_slice(&image.as_raw()[from..from + 4]);
        }
    }
    RgbaImage::from_raw(out_width, out_height, bytes).unwrap()
}

#[cfg(target_endian = "little")]
type NativeUploadCase = (DynamicImage, Vec<u8>);

#[cfg(target_endian = "little")]
struct RestoreTelemetry(bool);

#[cfg(target_endian = "little")]
impl Drop for RestoreTelemetry {
    fn drop(&mut self) {
        Backend::set_pipeline_telemetry_enabled(self.0);
    }
}

#[cfg(target_endian = "little")]
fn native_upload_test_cases(channels: usize, worker: usize) -> Vec<NativeUploadCase> {
    (0usize..6)
        .map(|iteration| {
            let (width, height) = [(129usize, 132usize), (132, 129), (131, 129)][iteration % 3];
            let bytes: Vec<u8> = (0..width * height * channels)
                .map(|index| {
                    index
                        .wrapping_mul(113)
                        .wrapping_add(index / 7)
                        .wrapping_add(worker * 61)
                        .wrapping_add(iteration * 43) as u8
                })
                .collect();
            let mut expected = vec![0; bytes.len()];
            for y in 0..height {
                for x in 0..width {
                    let input_start = (y * width + x) * channels;
                    let output_start = (x * height + y) * channels;
                    expected[output_start..output_start + channels]
                        .copy_from_slice(&bytes[input_start..input_start + channels]);
                }
            }
            (
                if channels == 3 {
                    DynamicImage::ImageRgb8(
                        RgbImage::from_raw(width as u32, height as u32, bytes).unwrap(),
                    )
                } else {
                    DynamicImage::ImageRgba8(
                        RgbaImage::from_raw(width as u32, height as u32, bytes).unwrap(),
                    )
                },
                expected,
            )
        })
        .collect()
}

#[cfg(target_endian = "little")]
fn execute_native_upload_worker(
    gpu: &'static super::GpuInner,
    mapped_input: bool,
    cases: Vec<NativeUploadCase>,
    start: &std::sync::Barrier,
) -> Vec<NativeUploadCase> {
    use crate::pipeline::TransposeMethod;

    let _telemetry = RestoreTelemetry(Backend::set_pipeline_telemetry_enabled(true));
    let mut buffers = super::BufferPool::new(&gpu.device, 132 * 132, true);
    let mut retained = Vec::new();
    start.wait();
    for (input, expected) in cases {
        let native_bytes = input.as_bytes().len();
        let transfer_bytes = (native_bytes as u64 + 3) & !3;
        let _ = crate::compute::take_pipeline_dispatch_count();
        let _ = crate::compute::take_pipeline_resource_telemetry();
        let _ = Backend::take_pipeline_allocation_telemetry();
        // This explicit GPU helper has no CPU fallback branch.
        let op = PipelineOp::Transpose {
            method: TransposeMethod::Transpose,
        };
        let actual = match &input {
            DynamicImage::ImageRgb8(rgb) => {
                let layout = super::packed_rgb_transpose_layout(
                    rgb.width(),
                    rgb.height(),
                    gpu.device.limits().max_compute_workgroups_per_dimension,
                )
                .unwrap();
                gpu.execute_packed_rgb_transpose_with_upload(
                    &op,
                    rgb,
                    &layout,
                    &mut buffers,
                    mapped_input,
                )
                .unwrap()
            }
            DynamicImage::ImageRgba8(rgba) => gpu
                .execute_rgba_transpose_with_upload(&op, rgba, &mut buffers, mapped_input)
                .unwrap(),
            _ => unreachable!(),
        };
        assert_eq!(actual.dimensions(), (input.height(), input.width()));
        assert_eq!(actual.color(), input.color());
        assert_eq!(actual.as_bytes(), expected);
        assert_eq!(crate::compute::take_pipeline_dispatch_count(), Some(1));
        let resources = crate::compute::take_pipeline_resource_telemetry().unwrap();
        assert_eq!(resources.upload_bytes, transfer_bytes);
        assert_eq!(resources.readback_bytes, transfer_bytes);
        assert_eq!(resources.mode_conversion_count, 0);
        assert_eq!(resources.full_frame_copy_count, u64::from(!mapped_input));
        assert_eq!(resources.retained_cache_bytes, buffers.retained_bytes());
        let allocations = Backend::take_pipeline_allocation_telemetry();
        assert_eq!(allocations.allocation_count, 1);
        assert_eq!(allocations.allocated_bytes, native_bytes as u64);
        retained.push((actual, expected));
    }
    retained
}

#[cfg(target_endian = "little")]
fn gpu_native_upload_paths_concurrent_reuse_and_resources(channels: usize) {
    let Some(gpu) = primary_readback_test_gpu() else {
        return;
    };
    // Compute independent reference samples before either worker starts.
    let jobs: Vec<_> = [false, true]
        .into_iter()
        .enumerate()
        .map(|(worker, mapped)| (mapped, native_upload_test_cases(channels, worker)))
        .collect();
    // Telemetry opt-in is thread-local, just like its result counters.
    let _telemetry = RestoreTelemetry(Backend::set_pipeline_telemetry_enabled(true));
    let start = std::sync::Barrier::new(2);
    let completed = std::thread::scope(|scope| {
        let handles: Vec<_> = jobs
            .into_iter()
            .map(|(mapped_input, cases)| {
                let start = &start;
                scope.spawn(move || execute_native_upload_worker(gpu, mapped_input, cases, start))
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("GPU upload worker"))
            .collect::<Vec<_>>()
    });
    // Later uploads, mappings and GPU writes must never alias an earlier
    // owned output, including after a worker's working set is dropped.
    for (actual, expected) in completed.into_iter().flatten() {
        assert_eq!(actual.as_bytes(), expected);
    }
}

fn primary_readback_test_gpu() -> Option<&'static super::GpuInner> {
    match super::GpuPool::ensure_init() {
        Ok(gpu) if gpu.direct_primary_readback => Some(gpu),
        Ok(_) => None,
        Err(error) if error.to_string().contains("GPU adapter not available") => None,
        Err(error) => panic!("GPU primary readback initialization failed: {error}"),
    }
}

// Use a private instance of the production pool on the shared test device.
// Tests cannot consume or clear another test's idle working sets.
fn isolated_working_set_test_gpu() -> Option<super::GpuInner> {
    let gpu = match super::GpuPool::ensure_init() {
        Ok(gpu) => gpu,
        Err(error) if error.to_string().contains("GPU adapter not available") => return None,
        Err(error) => panic!("native GPU pool initialization failed: {error}"),
    };
    Some(super::GpuInner {
        device: gpu.device.clone(),
        queue: gpu.queue.clone(),
        pipelines: std::sync::Mutex::new(std::collections::HashMap::new()),
        device_state: Arc::clone(&gpu.device_state),
        available_buffers: std::sync::Mutex::new(Vec::new()),
        available_staging: std::sync::Mutex::new(Vec::new()),
        direct_primary_readback: gpu.direct_primary_readback,
    })
}

fn actual_working_set_buffer_bytes(buffers: &super::BufferPool) -> u64 {
    let bytes = [
        &buffers.buf_a,
        &buffers.buf_b,
        &buffers.buf_img2,
        &buffers.buf_img3,
        &buffers.lut_buf,
        &buffers.histogram_buf,
        &buffers.params_arena.buffer,
        &buffers.img2_arena.buffer,
        &buffers.img3_arena.buffer,
        &buffers.lut_arena.buffer,
    ]
    .into_iter()
    .map(wgpu::Buffer::size)
    .sum::<u64>();
    #[cfg(target_endian = "little")]
    let bytes = bytes
        + buffers
            .mapped_native_input
            .as_ref()
            .map_or(0, |input| input.buffer.size());
    bytes
}

mod affine_geometry;
mod dispatch_planners;
mod native_cover;
mod native_effects;
mod native_filters;
mod native_layout_planners;
mod native_luma;
mod native_luma_transfer;
mod native_operations;
mod native_padding;
mod native_paste_execution;
mod native_paste_planning;
mod native_transform;
mod palette_projective;
mod pipeline_fusion;
mod point_merge;
mod projective_filtered;
mod projective_nearest_execution;
mod projective_nearest_execution_cases;
mod projective_nearest_proofs;
mod putalpha_planners;
mod readback;
mod resize_compact_box;
mod resize_float_execution;
mod resize_float_proofs;
mod resize_geometry;
mod resize_integer;
mod resize_ordered_float;
mod resize_ordered_taps;
mod resize_streaming;
mod resize_thumbnail;
mod resource_pool;
mod rgb_compact;
mod transpose_execution;
mod transpose_planning;
mod typed_geometry;
mod typed_resize;
mod upload_reuse;
