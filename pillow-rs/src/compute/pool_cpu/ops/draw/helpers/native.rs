use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{ImageBuffer, Luma, Pixel};

use super::{Luma16Canvas, NativeByteCanvas, draw_op_on_canvas};

// VERIFY: source=reviewed@2026-11-12; parity=pass@regular-polygon-prepush-final2-192x9; benchmark=pass@regular-polygon-standard-default-neon/pil-imagedraw-imagedraw.regular-polygon.standard
pub(crate) fn execute_byte_buffer<P, const CHANNELS: usize, const HIDDEN_LANES: usize>(
    image: &ImageBuffer<P, Vec<u8>>,
    ops: &[PipelineOp],
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    execute_native_buffer::<P, CHANNELS, HIDDEN_LANES>(image, ops)
}

// VERIFY: source=reviewed@2026-11-12; parity=pass@regular-polygon-prepush-final2-192x9; benchmark=pending
pub(crate) fn execute_float_buffer<P, const CHANNELS: usize, const HIDDEN_LANES: usize>(
    image: &ImageBuffer<P, Vec<u8>>,
    ops: &[PipelineOp],
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    execute_native_buffer::<P, CHANNELS, HIDDEN_LANES>(image, ops)
}

// VERIFY: source=reviewed@2026-11-12; parity=pass@regular-polygon-prepush-final2-192x9; benchmark=pass@regular-polygon-standard-default-neon/pil-imagedraw-imagedraw.regular-polygon.standard
fn execute_native_buffer<P, const CHANNELS: usize, const HIDDEN_LANES: usize>(
    image: &ImageBuffer<P, Vec<u8>>,
    ops: &[PipelineOp],
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    let mut output = image.clone();
    let mut hidden_lanes = output.take_hidden_pixel_lanes();
    let mut hidden_lane_update_error = None;
    {
        let mut canvas = NativeByteCanvas::<P, CHANNELS>::new(
            output.width(),
            output.height(),
            &mut output,
            &mut hidden_lanes,
            HIDDEN_LANES,
            &mut hidden_lane_update_error,
        );
        for op in ops {
            draw_op_on_canvas(&mut canvas, op)?;
        }
    }
    if let Some(error) = hidden_lane_update_error {
        return Err(error);
    }
    if !output.replace_hidden_pixel_lanes(hidden_lanes) {
        return Err(PilError::InternalError(
            "Draw hidden pixel lane extent did not match its image".into(),
        ));
    }
    Ok(output)
}

// VERIFY: source=reviewed@2026-11-12; parity=pass@regular-polygon-prepush-final2-192x9; benchmark=pending
pub(crate) fn execute_luma16_buffer<const DECLARED_BIG_ENDIAN: bool>(
    image: &ImageBuffer<Luma<u16>, Vec<u16>>,
    ops: &[PipelineOp],
) -> Result<ImageBuffer<Luma<u16>, Vec<u16>>, PilError> {
    let mut output = image.clone();
    {
        let mut canvas = Luma16Canvas::<DECLARED_BIG_ENDIAN>::new(&mut output);
        for op in ops {
            draw_op_on_canvas(&mut canvas, op)?;
        }
    }
    Ok(output)
}
