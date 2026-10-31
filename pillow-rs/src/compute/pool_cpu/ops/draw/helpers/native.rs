use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{ImageBuffer, Luma, Pixel};

use super::{Luma16Canvas, NativeByteCanvas, draw_op_on_canvas};

pub(crate) fn execute_byte_buffer<P, const CHANNELS: usize>(
    image: &ImageBuffer<P, Vec<u8>>,
    ops: &[PipelineOp],
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    execute_byte_buffer_with_mode::<P, CHANNELS, false>(image, ops)
}

pub(crate) fn execute_float_buffer<P, const CHANNELS: usize>(
    image: &ImageBuffer<P, Vec<u8>>,
    ops: &[PipelineOp],
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    execute_byte_buffer_with_mode::<P, CHANNELS, true>(image, ops)
}

fn execute_byte_buffer_with_mode<P, const CHANNELS: usize, const FLOAT_MODE: bool>(
    image: &ImageBuffer<P, Vec<u8>>,
    ops: &[PipelineOp],
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    let mut output = image.clone();
    {
        let mut canvas = NativeByteCanvas::<P, CHANNELS>::new(
            output.width(),
            output.height(),
            output.as_mut(),
        );
        for op in ops {
            draw_op_on_canvas::<_, FLOAT_MODE>(&mut canvas, op)?;
        }
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

