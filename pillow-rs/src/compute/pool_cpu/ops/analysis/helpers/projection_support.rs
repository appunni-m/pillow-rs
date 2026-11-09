//! GetProjection support declarations for exact-mode owners.

use crate::compute::registry::support::{ModeSupport, OperationId, OutputModeRule};
use crate::raster::Mode;

#[derive(Clone, Copy)]
pub(crate) enum SupportKind {
    Pillow,
    CoreOnly,
}

pub(crate) fn mode_support(mode: Mode, kind: SupportKind) -> ModeSupport {
    match kind {
        SupportKind::Pillow => ModeSupport::cpu(
            OperationId::GetProjection,
            mode,
            OutputModeRule::NoRasterResult,
            &[],
            &[],
        ),
        SupportKind::CoreOnly => ModeSupport::cpu_core_only(
            OperationId::GetProjection,
            mode,
            OutputModeRule::NoRasterResult,
            &[],
            &[],
            "native high-depth GetProjection extends the Pillow mode contract",
        ),
    }
}

macro_rules! declare_projection_support {
    ($mode:ident, $kind:ident $(,)?) => {
        pub(crate) fn get_projection_mode_support() ->
            $crate::compute::registry::support::ModeSupport
        {
            $crate::compute::pool_cpu::ops::analysis::helpers::projection_support::mode_support(
                $crate::raster::Mode::$mode,
                $crate::compute::pool_cpu::ops::analysis::helpers::projection_support::SupportKind::$kind,
            )
        }
    };
}

pub(crate) use declare_projection_support;
