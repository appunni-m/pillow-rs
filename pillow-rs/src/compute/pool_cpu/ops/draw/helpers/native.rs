use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{ImageBuffer, Luma, Pixel};

use super::{Luma16Canvas, NativeByteCanvas, draw_op_on_canvas};

pub(crate) fn execute_byte_buffer<P, const CHANNELS: usize, const HIDDEN_LANES: usize>(
    image: &ImageBuffer<P, Vec<u8>>,
    ops: &[PipelineOp],
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    execute_byte_buffer_with_mode::<P, CHANNELS, HIDDEN_LANES, false>(image, ops)
}

pub(crate) fn execute_float_buffer<P, const CHANNELS: usize, const HIDDEN_LANES: usize>(
    image: &ImageBuffer<P, Vec<u8>>,
    ops: &[PipelineOp],
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    execute_byte_buffer_with_mode::<P, CHANNELS, HIDDEN_LANES, true>(image, ops)
}

fn execute_byte_buffer_with_mode<
    P,
    const CHANNELS: usize,
    const HIDDEN_LANES: usize,
    const FLOAT_MODE: bool,
>(
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
            draw_op_on_canvas::<_, FLOAT_MODE>(&mut canvas, op)?;
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

pub(crate) fn execute_luma16_buffer<const DECLARED_BIG_ENDIAN: bool>(
    image: &ImageBuffer<Luma<u16>, Vec<u16>>,
    ops: &[PipelineOp],
) -> Result<ImageBuffer<Luma<u16>, Vec<u16>>, PilError> {
    let mut output = image.clone();
    {
        let mut canvas = Luma16Canvas::<DECLARED_BIG_ENDIAN>::new(&mut output);
        for op in ops {
            draw_op_on_canvas::<_, false>(&mut canvas, op)?;
        }
    }
    Ok(output)
}

