//! Queue submission and terminal-fence handling for explicit GPU batches.

use super::super::{
    MAX_GPU_OPS_PER_SUBMISSION, MAX_GPU_SHADER_WORK_ITEMS, StagingBuffer, aligned_bytes,
};
use super::{BatchError, BatchErrorKind, Flight, GpuBatchExecutor, Job, ReadbackArena, error};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

impl GpuBatchExecutor {
    pub(super) fn dispatch(&mut self) -> Result<(), BatchError> {
        while !self.pending.is_empty() && self.flights.len() < self.config.max_in_flight {
            let count = self.dispatch_chunk_count()?;
            let (mut jobs, mut commands, offsets, bytes) = self.take_pending_chunk(count);
            let staging = self.encode_terminal_readback(&mut jobs, &offsets, bytes, &mut commands);
            let fence = self.encode_completion_fence(&mut jobs, staging.as_ref(), &mut commands);
            self.submit_chunk(count, jobs, offsets, staging, fence, commands);
        }
        Ok(())
    }

    fn dispatch_chunk_count(&self) -> Result<usize, BatchError> {
        let mut count = 0;
        let mut chunk_bytes = 0u64;
        let mut chunk_work = 0u64;
        let mut chunk_ops = 0usize;
        let max_buffer_size = self.gpu.device.limits().max_buffer_size;
        for job in &self.pending {
            let Some(bytes) = chunk_bytes.checked_add(aligned_bytes(job.length, 8) as u64) else {
                break;
            };
            let Some(work) = chunk_work.checked_add(job.shader_work) else {
                break;
            };
            let ops = chunk_ops.saturating_add(job.op_count);
            if count > 0
                && (bytes > max_buffer_size
                    || work > MAX_GPU_SHADER_WORK_ITEMS
                    || ops > MAX_GPU_OPS_PER_SUBMISSION)
            {
                break;
            }
            if bytes > max_buffer_size {
                return Err(error(
                    Some(job.id),
                    job.key.clone(),
                    "dispatch",
                    BatchErrorKind::Limit,
                    "terminal arena exceeds device buffer limit",
                ));
            }
            (chunk_bytes, chunk_work, chunk_ops) = (bytes, work, ops);
            count += 1;
            if count >= self.config.max_jobs.div_ceil(self.config.max_in_flight) {
                break;
            }
            if !self.config.queue {
                break;
            }
        }
        Ok(count)
    }

    fn take_pending_chunk(
        &mut self,
        count: usize,
    ) -> (Vec<Job>, Vec<wgpu::CommandBuffer>, Vec<u64>, u64) {
        let mut jobs: Vec<_> = self.pending.drain(..count).collect();
        let mut commands = Vec::with_capacity(count + 1);
        let mut offsets = Vec::with_capacity(count);
        let mut bytes = 0u64;
        for job in &mut jobs {
            offsets.push(bytes);
            bytes += aligned_bytes(job.length, 8) as u64;
            commands.push(
                job.command
                    .take()
                    .expect("unsubmitted job owns its command buffer"),
            );
            self.stats.dispatches += job.dispatches;
            self.stats.upload_bytes += job.upload;
        }
        (jobs, commands, offsets, bytes)
    }

    fn encode_terminal_readback(
        &mut self,
        jobs: &mut [Job],
        offsets: &[u64],
        bytes: u64,
        commands: &mut Vec<wgpu::CommandBuffer>,
    ) -> Option<Arc<ReadbackArena>> {
        if self.resident {
            return None;
        }
        // Each job reserved an eight-byte-aligned terminal arena share at
        // admission. Moving that share keeps the physical arena charged until
        // its last ready range is consumed.
        let mut credit = jobs[0].credit.split(0, 0);
        for job in jobs.iter_mut() {
            let mut part = job.credit.split(
                aligned_bytes(job.length, 8) as u64,
                aligned_bytes(job.length, 8) as u64,
            );
            credit.gpu += part.gpu;
            credit.host += part.host;
            part.gpu = 0;
            part.host = 0; // transfer the charge; dropping part releases its Arc
        }
        let buffer = StagingBuffer::new(&self.gpu.device, bytes).buffer;
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gpu_stream_terminal_copies"),
            });
        for (job, &offset) in jobs.iter().zip(offsets) {
            encoder.copy_buffer_to_buffer(
                job.output.as_ref().expect("terminal buffer retained"),
                0,
                &buffer,
                offset,
                aligned_bytes(job.length, 4) as u64,
            );
        }
        commands.push(encoder.finish());
        self.stats.readback_bytes += jobs.iter().map(|job| job.length as u64).sum::<u64>();
        Some(Arc::new(ReadbackArena {
            buffer,
            bytes,
            mapped: AtomicBool::new(false),
            _credit: credit,
        }))
    }

    fn encode_completion_fence(
        &self,
        jobs: &mut [Job],
        staging: Option<&Arc<ReadbackArena>>,
        commands: &mut Vec<wgpu::CommandBuffer>,
    ) -> Arc<ReadbackArena> {
        if let Some(arena) = staging {
            return arena.clone();
        }
        let credit = jobs[0].credit.split(8, 8);
        let buffer = StagingBuffer::new(&self.gpu.device, 8).buffer;
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gpu_stream_resident_fence"),
            });
        encoder.copy_buffer_to_buffer(
            jobs[0].output.as_ref().expect("terminal buffer retained"),
            0,
            &buffer,
            0,
            4,
        );
        commands.push(encoder.finish());
        Arc::new(ReadbackArena {
            buffer,
            bytes: 8,
            mapped: AtomicBool::new(false),
            _credit: credit,
        })
    }

    fn submit_chunk(
        &mut self,
        count: usize,
        jobs: Vec<Job>,
        offsets: Vec<u64>,
        staging: Option<Arc<ReadbackArena>>,
        fence: Arc<ReadbackArena>,
        commands: Vec<wgpu::CommandBuffer>,
    ) {
        self.gpu.queue.submit(commands);
        // Buffer mapping is a fence for this chunk's last buffer use. A
        // queue-wide callback could accidentally wait for unrelated work
        // submitted concurrently after ours on Pillow's shared queue.
        let completed = Arc::new(AtomicBool::new(false));
        let failure = Arc::new(Mutex::new(None));
        let flag = completed.clone();
        let failed = failure.clone();
        let mapped_arena = fence.clone();
        fence
            .buffer
            .slice(..fence.bytes)
            .map_async(wgpu::MapMode::Read, move |result| {
                match result {
                    Ok(()) => mapped_arena.mapped.store(true, Ordering::Release),
                    Err(error) => {
                        *failed.lock().unwrap_or_else(|e| e.into_inner()) = Some(error.to_string())
                    }
                }
                flag.store(true, Ordering::Release);
            });
        self.stats.submitted_jobs += count as u64;
        self.stats.submissions += 1;
        self.stats.largest_submission = self.stats.largest_submission.max(count);
        self.flights.push_back(Flight {
            jobs,
            completed,
            failure,
            _fence: fence,
            staging,
            offsets,
            submitted_at: Instant::now(),
        });
    }
}
