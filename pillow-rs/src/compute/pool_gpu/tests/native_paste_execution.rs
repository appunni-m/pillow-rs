use super::*;

#[test]
#[cfg(target_endian = "little")]
fn native_masked_rgba_paste_matches_cpu_with_compact_gpu_shader() {
    use crate::ops::paste::PasteSource;

    match super::GpuPool::ensure_init() {
        Ok(_) => {}
        Err(error)
            if error.to_string().contains("GPU adapter not available")
                || error
                    .to_string()
                    .contains("GPU device initialization failed") =>
        {
            return;
        }
        Err(error) => panic!("native masked RGBA GPU initialization failed: {error}"),
    }

    struct RestoreDiagnostics {
        telemetry: bool,
        shader_coverage: bool,
    }
    impl Drop for RestoreDiagnostics {
        fn drop(&mut self) {
            Backend::set_pipeline_telemetry_enabled(self.telemetry);
            Backend::set_gpu_shader_coverage_enabled(self.shader_coverage);
        }
    }
    let _restore = RestoreDiagnostics {
        telemetry: Backend::set_pipeline_telemetry_enabled(true),
        shader_coverage: Backend::set_gpu_shader_coverage_enabled(true),
    };
    let _ = Backend::take_gpu_shader_coverage();

    let (source_width, source_height) = (5usize, 3usize);
    let (destination_width, destination_height) = (7usize, 4usize);
    let destination_bytes = (0..destination_width * destination_height * 4)
        .map(|index| (index * 41 + 19) as u8)
        .collect::<Vec<_>>();
    let source_bytes = (0..source_width * source_height * 4)
        .map(|index| (index * 73 + 7) as u8)
        .collect::<Vec<_>>();
    let mask_pattern = [0u8, 1, 127, 128, 254, 255];
    let mask_bytes = (0..source_width * source_height)
        .map(|index| mask_pattern[index % mask_pattern.len()])
        .collect::<Vec<_>>();
    let positions = [(-1, -1), (2, 1)];
    for position in positions {
        let source = Image::frombytes("RGBA", (5, 3), &source_bytes).unwrap();
        let mask = Image::frombytes("L", (5, 3), &mask_bytes).unwrap();
        let mut expected_image = Image::frombytes("RGBA", (7, 4), &destination_bytes).unwrap();
        expected_image
            .paste_at(
                PasteSource::Image(Box::new(source.clone())),
                Some(position),
                Some(&mask),
            )
            .unwrap();
        let expected = expected_image
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU masked RGBA Paste reference");

        let mut actual_image = Image::frombytes("RGBA", (7, 4), &destination_bytes).unwrap();
        actual_image
            .paste_at(
                PasteSource::Image(Box::new(source)),
                Some(position),
                Some(&mask),
            )
            .unwrap();
        let actual = actual_image
            .use_backend(Backend::Gpu)
            .tobytes()
            .expect("native GPU masked RGBA Paste");
        assert_eq!(actual, expected, "masked RGBA Paste at {position:?}");

        let receipt = Backend::take_pipeline_telemetry()
            .expect("native RGBA Paste must publish GPU telemetry");
        assert_eq!(receipt.0, Some(Backend::Gpu));
        assert_eq!(receipt.1, Backend::Gpu);
        assert_eq!(receipt.6, Some(1));
        assert_eq!(receipt.7, None);
        let resources = receipt.8.expect("native masked RGBA GPU resources");
        assert_eq!(resources.mode_conversion_count, 0);
        assert_eq!(resources.upload_bytes, 112 + 60 + 16);
        assert_eq!(resources.auxiliary_bytes, 60 + 16);
        assert_eq!(resources.readback_bytes, 112);

        let shader_dispatches = Backend::take_gpu_shader_coverage();
        assert_eq!(shader_dispatches.len(), 1);
        assert_eq!(
            shader_dispatches[0].variant_name,
            "__internal_paste_native_masked_rgba"
        );
        assert_eq!(
            shader_dispatches[0].shader_file,
            "paste_native_masked_rgba.wgsl"
        );
        assert_eq!(shader_dispatches[0].dispatches, 1);
        assert_eq!(shader_dispatches[0].workgroups, 1);
    }
}

#[test]
#[cfg(target_endian = "little")]
fn native_masked_rgba_paste_shader_covers_a_two_dimensional_grid() {
    match super::GpuPool::ensure_init() {
        Ok(_) => {}
        Err(error)
            if error.to_string().contains("GPU adapter not available")
                || error
                    .to_string()
                    .contains("GPU device initialization failed") =>
        {
            return;
        }
        Err(error) => panic!("native masked RGBA GPU initialization failed: {error}"),
    }
    let gpu = super::GpuPool::ensure_init().expect("GPU initialized above");
    let (width, height) = (129u32, 1u32);
    let source_bytes = (0..width as usize * 4)
        .map(|index| {
            u8::try_from(index % 256)
                .expect("wrapped source sample fits in a byte")
                .wrapping_mul(31)
                .wrapping_add(7)
        })
        .collect::<Vec<_>>();
    let destination_bytes = (0..width as usize * 4)
        .map(|index| {
            u8::try_from(index % 256)
                .expect("wrapped destination sample fits in a byte")
                .wrapping_mul(47)
                .wrapping_add(19)
        })
        .collect::<Vec<_>>();
    let mask_pattern = [0u8, 1, 127, 128, 254, 255];
    let mask_bytes = (0..width as usize)
        .map(|index| mask_pattern[index % mask_pattern.len()])
        .collect::<Vec<_>>();
    let mut expected = destination_bytes.clone();
    for pixel in 0..width as usize {
        let mask = u32::from(mask_bytes[pixel]);
        for channel in 0..4 {
            let index = pixel * 4 + channel;
            let source = u32::from(source_bytes[index]);
            let destination = u32::from(expected[index]);
            expected[index] =
                u8::try_from((source * mask + destination * (255 - mask) + 127) / 255)
                    .expect("the rounded blend remains an 8-bit sample");
        }
    }

    let destination = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(width, height, destination_bytes).expect("RGBA destination"),
    );
    let source = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(width, height, source_bytes).expect("RGBA source"),
    );
    let mask =
        DynamicImage::ImageLuma8(GrayImage::from_raw(width, height, mask_bytes).expect("L mask"));
    let limits = gpu.device.limits();
    let dispatch = plan_gpu_native_masked_byte_paste(
        width,
        height,
        width,
        height,
        width,
        height,
        4,
        2,
        limits.max_storage_buffer_binding_size,
        limits.max_buffer_size,
    )
    .expect("the synthetic two-workgroup axis limit yields a two-dimensional grid");
    assert_eq!((dispatch.workgroups_x, dispatch.workgroups_y), (2, 2));

    let mut buffers = gpu.acquire_buffers(dispatch.word_count).unwrap();
    let actual = gpu
        .execute_native_masked_byte_paste(
            &destination,
            &source,
            &mask,
            "RGBA",
            (0, 0),
            dispatch,
            &mut buffers,
        )
        .expect("the bounded 2×2 GPU dispatch completes");
    gpu.recycle_buffers(buffers);

    assert_eq!(actual.as_bytes(), expected);
}

#[test]
#[cfg(target_endian = "little")]
fn native_masked_pa_paste_executes_the_pa_shader_without_color_conversion() {
    use crate::ops::paste::PasteSource;

    match super::GpuPool::ensure_init() {
        Ok(_) => {}
        Err(error)
            if error.to_string().contains("GPU adapter not available")
                || error
                    .to_string()
                    .contains("GPU device initialization failed") =>
        {
            return;
        }
        Err(error) => panic!("native masked PA GPU initialization failed: {error}"),
    }

    let palette = [10, 20, 30, 40, 50, 60];
    let source_bytes = (0usize..5 * 3 * 2)
        .map(|index| index.wrapping_mul(37).wrapping_add(index / 2 + 13) as u8)
        .collect::<Vec<_>>();
    let destination_bytes = (0usize..5 * 3 * 2)
        .map(|index| index.wrapping_mul(53).wrapping_add(index / 3 + 7) as u8)
        .collect::<Vec<_>>();
    let mask_bytes = vec![
        0, 1, 127, 128, 254, 255, 127, 128, 1, 254, 0, 255, 128, 1, 254,
    ];
    let mut source = Image::frombytes("PA", (5, 3), &source_bytes).expect("PA source");
    source
        .putpalette(&palette, "RGB")
        .expect("PA source palette");
    let mut expected_image =
        Image::frombytes("PA", (5, 3), &destination_bytes).expect("PA destination reference");
    expected_image
        .putpalette(&palette, "RGB")
        .expect("PA destination palette");
    let mask = Image::frombytes("L", (5, 3), &mask_bytes).expect("L mask");
    let position = (-1, -1);
    expected_image
        .paste_at(
            PasteSource::Image(Box::new(source.clone())),
            Some(position),
            Some(&mask),
        )
        .expect("CPU PA Paste reference");
    let expected = expected_image
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU masked PA Paste");

    let mut actual_image =
        Image::frombytes("PA", (5, 3), &destination_bytes).expect("PA GPU destination");
    actual_image
        .putpalette(&palette, "RGB")
        .expect("PA GPU destination palette");
    actual_image
        .paste_at(
            PasteSource::Image(Box::new(source)),
            Some(position),
            Some(&mask),
        )
        .expect("queue PA GPU Paste");
    let gpu_image = actual_image.use_backend(Backend::Gpu);
    assert_eq!(gpu_image.mode().expect("PA output mode"), "PA");
    assert_eq!(gpu_image.palette(), Some(palette.to_vec()));
    let actual = gpu_image.tobytes().expect("native GPU masked PA Paste");
    assert_eq!(actual, expected, "masked PA Paste output");
}

#[test]
#[cfg(target_endian = "little")]
fn native_rgb_crop_uses_its_shader_and_preserves_odd_output_tail() {
    match super::GpuPool::ensure_init() {
        Ok(_) => {}
        Err(error)
            if error.to_string().contains("GPU adapter not available")
                || error
                    .to_string()
                    .contains("GPU device initialization failed") =>
        {
            return;
        }
        Err(error) => panic!("native RGB Crop GPU initialization failed: {error}"),
    }

    struct RestoreDiagnostics {
        telemetry: bool,
        shader_coverage: bool,
    }
    impl Drop for RestoreDiagnostics {
        fn drop(&mut self) {
            Backend::set_pipeline_telemetry_enabled(self.telemetry);
            Backend::set_gpu_shader_coverage_enabled(self.shader_coverage);
        }
    }
    let _restore = RestoreDiagnostics {
        telemetry: Backend::set_pipeline_telemetry_enabled(true),
        shader_coverage: Backend::set_gpu_shader_coverage_enabled(true),
    };
    let _ = Backend::take_pipeline_telemetry();
    let _ = Backend::take_gpu_shader_coverage();

    let source_bytes = (0..45)
        .map(|index| (index * 29 + 17) as u8)
        .collect::<Vec<_>>();
    let source = Image::frombytes("RGB", (5, 3), &source_bytes).expect("RGB source");
    let expected = source
        .crop(Some((1, 1, 4, 3)))
        .expect("RGB crop")
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU RGB Crop reference");
    let _ = Backend::take_pipeline_telemetry();

    let actual = source
        .crop(Some((1, 1, 4, 3)))
        .expect("RGB crop")
        .use_backend(Backend::Gpu)
        .tobytes()
        .expect("native GPU RGB Crop");
    assert_eq!(actual, expected, "native RGB Crop pixel parity");

    let receipt =
        Backend::take_pipeline_telemetry().expect("native RGB Crop must publish telemetry");
    assert_eq!(receipt.0, Some(Backend::Gpu));
    assert_eq!(receipt.1, Backend::Gpu);
    assert_eq!(receipt.6, Some(1));
    assert_eq!(receipt.7, None);
    let resources = receipt.8.expect("native RGB Crop resource telemetry");
    assert_eq!(resources.upload_bytes, 48);
    assert_eq!(resources.readback_bytes, 20);
    assert_eq!(resources.mode_conversion_count, 0);

    let shader_dispatches = Backend::take_gpu_shader_coverage();
    assert_eq!(shader_dispatches.len(), 1);
    assert_eq!(
        shader_dispatches[0].variant_name,
        "__internal_crop_native_rgb"
    );
    assert_eq!(shader_dispatches[0].shader_file, "crop_native_rgb.wgsl");
    assert_eq!(shader_dispatches[0].dispatches, 1);
    assert_eq!(shader_dispatches[0].workgroups, 1);
}
