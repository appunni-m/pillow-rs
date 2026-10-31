//! NEON strategy wrapper around the shared byte-gather kernel.

use crate::error::PilError;

pub(crate) fn execute(source: &[u8], dest_map: &[u8]) -> Result<Vec<u8>, PilError> {
    super::super::super::helpers::remap_palette::vector(source, dest_map)
}
