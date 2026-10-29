//! Existing vector write regressions, adapted to exact modes and prepared context.

use super::super::ops::{self, PixelWrite};
use crate::compute::{Architecture, Backend, ExecutionContext};
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, ImageBuffer, RgbImage, Rgba};

fn put_pixel(x: u32, y: u32, color: [u8; 4]) -> PipelineOp {
    PipelineOp::PutPixel {
        x, y, color: (color[0], color[1], color[2], color[3]), palette_index: false,
    }
}

fn vector_batch_admitted(
    source: &DynamicImage, writes: &[PipelineOp], ctx: &ExecutionContext,
) -> bool {
    ops::prepare_put_pixel_batch(source, writes, ctx).unwrap()
        .selection().implementation.architecture != Architecture::Default
}

#[test]
fn rgba_putpixel_batch_matches_ordered_single_writes_at_vector_edges() {
    let ctx = ExecutionContext::new(Backend::Cpu, Architecture::detected_host());
    for (width, height) in [(4, 1), (1, 4), (17, 3), (31, 5)] {
        let source = DynamicImage::ImageRgba8(ImageBuffer::from_fn(width, height, |x, y| {
            Rgba([(x * 7) as u8, (y * 13) as u8, (x + y) as u8, 255])
        }));
        let writes = vec![
            put_pixel(0, 0, [1, 2, 3, 4]),
            put_pixel(width - 1, height - 1, [5, 6, 7, 8]),
            put_pixel(width / 2, height / 2, [9, 10, 11, 12]),
            put_pixel(0, 0, [13, 14, 15, 16]),
        ];
        assert!(vector_batch_admitted(&source, &writes, &ctx));
        let batched = ops::prepare_put_pixel_batch(&source, &writes, &ctx)
            .unwrap().execute().unwrap();
        let mut sequential = source;
        for op in &writes {
            let PipelineOp::PutPixel { x, y, color, .. } = op else {
                unreachable!("test fixture contains only PutPixel operations");
            };
            sequential = ops::prepare_put_pixel(
                &sequential, PixelWrite { x: *x, y: *y, color: *color }, &ctx,
            ).unwrap().execute().unwrap();
        }
        assert_eq!(batched.as_bytes(), sequential.as_bytes());
    }
}

#[test]
fn rgba_putpixel_batch_rejects_unmatched_or_unsafe_inputs() {
    let ctx = ExecutionContext::new(Backend::Cpu, Architecture::detected_host());
    let source = DynamicImage::ImageRgba8(ImageBuffer::new(4, 1));
    let two_writes = vec![put_pixel(0, 0, [1, 2, 3, 4]), put_pixel(3, 0, [5, 6, 7, 8])];
    // The former Some("RGB") override is now the exact RGB identity. Retain
    // the bytes, shape, and assertion that this is ineligible for the RGBA run.
    let rgb = DynamicImage::ImageRgb8(
        RgbImage::from_raw(4, 1, source.as_bytes().to_vec()).unwrap(),
    );
    assert!(!vector_batch_admitted(&rgb, &two_writes, &ctx));
    assert!(!vector_batch_admitted(&source, &two_writes[..1], &ctx));
    let out_of_bounds = vec![put_pixel(0, 0, [1, 2, 3, 4]), put_pixel(4, 0, [5, 6, 7, 8])];
    assert!(!vector_batch_admitted(&source, &out_of_bounds, &ctx));
    let tiny = DynamicImage::ImageRgba8(ImageBuffer::new(3, 1));
    assert!(!vector_batch_admitted(&tiny, &two_writes, &ctx));
}
