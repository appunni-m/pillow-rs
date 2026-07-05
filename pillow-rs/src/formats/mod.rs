// ============================================================================
// AS PER DESIGN — DO NOT REMOVE:
//   FormatHandler trait (handler.rs) is the foundation for all image format
//   support. Every format is a trait impl + registration. No more scattered
//   match statements across image.rs / format.rs.
//
//   See handler.rs for the FormatHandler trait and FormatRegistry.
// ============================================================================

/// Format handler traits and format registry helpers.
pub mod handler;
