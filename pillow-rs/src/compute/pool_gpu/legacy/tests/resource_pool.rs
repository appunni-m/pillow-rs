use super::*;

#[test]
fn gpu_working_set_pool_reuses_four_production_leases() {
    let Some(gpu) = isolated_working_set_test_gpu() else {
        return;
    };
    assert_eq!(super::MAX_RETAINED_GPU_WORKING_SETS, 4);
    assert_eq!(super::MAX_RETAINED_GPU_WORKING_BYTES, 128 * 1024 * 1024);
    // Acquire four simultaneously, as four in-flight materializations do.
    // Tiny real buffers make this a count/reuse regression, not a memory
    // stress test. Handle equality proves reuse without invented counters.
    let leases: Vec<_> = (64..68)
        .map(|capacity| gpu.acquire_buffers(capacity).unwrap())
        .collect();
    let identities: Vec<_> = leases
        .iter()
        .map(|buffers| (buffers.buf_a.clone(), buffers.buf_b.clone()))
        .collect();
    let expected_bytes: u64 = leases
        .iter()
        .map(|buffers| {
            let actual = actual_working_set_buffer_bytes(buffers);
            assert_eq!(buffers.retained_bytes(), actual);
            actual
        })
        .sum();
    for buffers in leases {
        gpu.recycle_buffers(buffers);
    }
    {
        let idle = gpu.available_buffers.lock().unwrap();
        assert_eq!(idle.len(), 4);
        assert_eq!(
            idle.iter()
                .map(super::BufferPool::retained_bytes)
                .sum::<u64>(),
            expected_bytes
        );
    }
    let reused: Vec<_> = (0..4).map(|_| gpu.acquire_buffers(64).unwrap()).collect();
    assert!(gpu.available_buffers.lock().unwrap().is_empty());
    for (index, buffers) in reused.iter().enumerate() {
        assert_eq!(buffers.capacity, 64 + index as u32);
        assert_eq!(buffers.buf_a, identities[index].0);
        assert_eq!(buffers.buf_b, identities[index].1);
    }
    for buffers in reused {
        gpu.recycle_buffers(buffers);
    }
    // A fifth idle set must not expand the count bound. The sorted policy
    // keeps the four smaller working sets and evicts this larger one.
    let fifth = super::BufferPool::new(&gpu.device, 68, gpu.direct_primary_readback);
    gpu.recycle_buffers(fifth);
    let idle = gpu.available_buffers.lock().unwrap();
    assert_eq!(idle.len(), 4);
    assert_eq!(
        idle.iter()
            .map(|buffers| buffers.capacity)
            .collect::<Vec<_>>(),
        vec![64, 65, 66, 67]
    );
    assert_eq!(
        idle.iter()
            .map(super::BufferPool::retained_bytes)
            .sum::<u64>(),
        expected_bytes
    );
}

#[test]
fn gpu_working_set_pool_scaled_byte_boundaries_reject_and_evict() {
    let Some(gpu) = isolated_working_set_test_gpu() else {
        return;
    };
    assert_eq!(super::MAX_RETAINED_GPU_WORKING_BYTES, 128 * 1024 * 1024);
    // Exercise the exact production recycle body with a scaled byte cap.
    // All totals come from real buffer sizes; no allocation metadata is
    // fabricated, and testing the boundary does not allocate 128 MiB.
    let first = super::BufferPool::new(&gpu.device, 64, gpu.direct_primary_readback);
    let first_bytes = actual_working_set_buffer_bytes(&first);
    assert_eq!(first.retained_bytes(), first_bytes);
    gpu.recycle_buffers_with_limits(first, 4, first_bytes - 1);
    assert!(
        gpu.available_buffers.lock().unwrap().is_empty(),
        "one oversized set is rejected"
    );

    let first = super::BufferPool::new(&gpu.device, 64, gpu.direct_primary_readback);
    gpu.recycle_buffers_with_limits(first, 4, first_bytes);
    assert_eq!(
        gpu.available_buffers.lock().unwrap().len(),
        1,
        "a set exactly at the limit is retained"
    );

    let mut second = super::BufferPool::new(&gpu.device, 65, gpu.direct_primary_readback);
    second.params_arena.ensure_capacity(
        &gpu.device,
        "gpu_pool_byte_boundary_test",
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        512,
        4,
    );
    let second_bytes = actual_working_set_buffer_bytes(&second);
    assert_eq!(
        second.retained_bytes(),
        second_bytes,
        "grown arena storage counts toward the bound"
    );
    let aggregate_limit = first_bytes + second_bytes;
    gpu.recycle_buffers_with_limits(second, 4, aggregate_limit);
    {
        let idle = gpu.available_buffers.lock().unwrap();
        assert_eq!(idle.len(), 2);
        assert_eq!(
            idle.iter()
                .map(super::BufferPool::retained_bytes)
                .sum::<u64>(),
            aggregate_limit
        );
    }
    assert_working_set_pool_aggregate_eviction(&gpu, aggregate_limit, first_bytes);
}

fn assert_working_set_pool_aggregate_eviction(
    gpu: &super::GpuInner,
    aggregate_limit: u64,
    first_bytes: u64,
) {
    let third = super::BufferPool::new(&gpu.device, 66, gpu.direct_primary_readback);
    let third_bytes = actual_working_set_buffer_bytes(&third);
    assert_eq!(third.retained_bytes(), third_bytes);
    assert!(
        third_bytes < aggregate_limit,
        "the aggregate guard, not the per-set guard, must evict"
    );
    gpu.recycle_buffers_with_limits(third, 4, aggregate_limit);
    {
        let idle = gpu.available_buffers.lock().unwrap();
        assert_eq!(
            idle.iter()
                .map(|buffers| buffers.capacity)
                .collect::<Vec<_>>(),
            vec![64, 65]
        );
        assert_eq!(
            idle.iter()
                .map(super::BufferPool::retained_bytes)
                .sum::<u64>(),
            aggregate_limit
        );
    }
    let fourth = super::BufferPool::new(&gpu.device, 67, gpu.direct_primary_readback);
    gpu.recycle_buffers_with_limits(fourth, 4, aggregate_limit - 1);
    let idle = gpu.available_buffers.lock().unwrap();
    assert_eq!(idle.len(), 1);
    assert_eq!(idle[0].capacity, 64);
    assert_eq!(idle[0].retained_bytes(), first_bytes);
}

#[test]
fn gpu_buffer_reuse_rejects_oversized_working_sets() {
    assert!(gpu_buffer_reuse_allowed(3_072, 768));
    assert!(gpu_buffer_reuse_allowed(768, 768));
    assert!(!gpu_buffer_reuse_allowed(3_073, 768));
    assert!(!gpu_buffer_reuse_allowed(786_432, 768));
    assert!(gpu_buffer_reuse_allowed(u32::MAX, 0));
    assert!(!gpu_buffer_reuse_allowed(u32::MAX, 1));
}

#[cfg(target_endian = "little")]
#[test]
fn rgb_staging_expansion_matches_dynamic_image_conversion() {
    let cases = [
        (0, 0, Vec::new()),
        (1, 1, vec![0, 127, 255]),
        (2, 1, vec![0, 0, 0, 255, 255, 255]),
        (
            3,
            2,
            vec![
                1, 2, 3, 4, 5, 6, 7, 8, 9, 240, 241, 242, 253, 254, 255, 0, 0, 0,
            ],
        ),
    ];

    for (width, height, source) in cases {
        let image = RgbImage::from_raw(width, height, source.clone()).unwrap();
        let expected = DynamicImage::ImageRgb8(image).to_rgba8().into_raw();
        let mut actual = vec![0; expected.len()];
        expand_rgb_into_rgba(&source, &mut actual).unwrap();
        assert_eq!(actual, expected);
    }
}

#[cfg(target_endian = "little")]
#[test]
fn rgb_staging_expansion_rejects_invalid_lengths() {
    assert!(expand_rgb_into_rgba(&[1, 2], &mut [0; 4]).is_err());
    assert!(expand_rgb_into_rgba(&[1, 2, 3], &mut [0; 3]).is_err());
    assert!(expand_rgb_into_rgba(&[1, 2, 3], &mut [0; 8]).is_err());
}

#[test]
fn readback_poll_backoff_is_bounded_by_interval_and_deadline() {
    let now = Instant::now();
    assert_eq!(
        readback_poll_backoff(true, 1, now, now + Duration::from_millis(2)),
        Some(GPU_POLL_FAST_BACKOFF)
    );
    assert_eq!(
        readback_poll_backoff(
            true,
            GPU_POLL_FAST_RETRIES + 1,
            now,
            now + Duration::from_millis(2)
        ),
        Some(GPU_POLL_BACKOFF)
    );

    let final_backoff = Duration::from_micros(250);
    assert_eq!(
        readback_poll_backoff(false, 1, now, now + Duration::from_millis(2)),
        Some(GPU_POLL_BACKOFF)
    );
    assert_eq!(
        readback_poll_backoff(false, GPU_POLL_FAST_RETRIES + 1, now, now + final_backoff),
        Some(final_backoff)
    );
    assert_eq!(readback_poll_backoff(true, 1, now, now), None);
    assert_eq!(
        readback_poll_backoff(true, 1, now + Duration::from_nanos(1), now),
        None
    );
}
