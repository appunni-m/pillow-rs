//! Eligibility guard for Draw's migrated native-byte span kernels.

use super::super::{DrawRequestKind, GuardAssessment, RequestFacts, StrategyInputs};

pub(crate) fn draw_native_spans_guard(input: StrategyInputs) -> GuardAssessment {
    match input.request {
        RequestFacts::Draw {
            kind: DrawRequestKind::GeometryBatch,
            all_geometry_spans_eligible: true,
            ..
        } => GuardAssessment::Satisfied,
        RequestFacts::Draw {
            kind: DrawRequestKind::GeometryBatch,
            all_geometry_spans_eligible: false,
            ..
        }
        | RequestFacts::Draw {
            kind: DrawRequestKind::Bitmap | DrawRequestKind::Text,
            ..
        } => GuardAssessment::Ineligible,
        RequestFacts::Unspecified => GuardAssessment::Deferred,
        _ => GuardAssessment::Ineligible,
    }
}
