//! ImageRgb8 support and serial Default selection for ConvertLab.

use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OutputModeRule,
};
use crate::raster::Mode;

const INPUTS: &[InputContract] = super::super::ops::RGB_INPUTS;

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::ConvertLab,
    Mode::ImageRgb8,
    OutputModeRule::Fixed(Mode::ImageLab8),
    INPUTS,
    &[],
);
