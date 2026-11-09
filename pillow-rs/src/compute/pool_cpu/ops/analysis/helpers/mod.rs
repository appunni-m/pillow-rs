//! Shared mechanics for direct analysis operations.

pub(crate) mod bbox;
pub(crate) mod bbox_execution;
pub(crate) mod extrema;
pub(crate) mod extrema_bytes;
pub(crate) mod extrema_bytes_execution;
pub(crate) mod extrema_execution;
pub(crate) mod get_channel_support;
pub(crate) mod get_data;
pub(crate) mod get_data_support;
pub(crate) mod getcolors;
pub(crate) mod getcolors_execution;
pub(crate) mod getcolors_support;
pub(crate) mod histogram_execution;
pub(crate) mod histogram_samples;
pub(crate) mod pixel_access;
pub(crate) mod projection;
pub(crate) mod projection_support;
pub(crate) mod projection_vector;
pub(crate) mod split_support;
pub(crate) mod stat;
pub(crate) mod stat_execution;
