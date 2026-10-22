use super::{Image, PilError};
use crate::compute::Backend;
use crate::pipeline::{PipelineOp, PixelMode};
use bytes::Bytes;

#[test]
fn native_l_putdata_keeps_the_shared_allocation_and_materializes_exactly() -> Result<(), PilError> {
    let payload = Bytes::from_static(&[13, 71, 209, 255]);
    let payload_ptr = payload.as_ptr();
    let mut image = Image::new(4, 1, "L", (0, 0, 0, 255))?;

    assert!(image.putdata_shared_bytes_fast_path(payload, 4, 1.0, 0.0)?);
    let queued_ptr = match &image {
        Image::Pipeline { ops, .. } => match &ops.as_slice()[0] {
            PipelineOp::PutData {
                data,
                mode: PixelMode::L,
            } => data.as_ptr(),
            other => panic!("unexpected queued operation: {other:?}"),
        },
        other => panic!("putdata did not queue a pipeline: {other:?}"),
    };
    assert_eq!(queued_ptr, payload_ptr);

    assert_eq!(
        image.use_backend(Backend::Cpu).tobytes()?,
        [13, 71, 209, 255]
    );
    Ok(())
}
