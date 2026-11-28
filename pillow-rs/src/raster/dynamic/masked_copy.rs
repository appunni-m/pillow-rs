use super::super::buffer::HiddenPixelLanes;
use super::DynamicImage;
use crate::raster::Mode;

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(super) fn merge_hidden_lanes(
    destination: &DynamicImage,
    source: &DynamicImage,
    x: u32,
    y: u32,
    mask: &[u8],
) -> Option<Option<HiddenPixelLanes>> {
    let mode = destination.mode();
    let Some(lane_count) = mode.hidden_pixel_lane_count() else {
        return Some(None);
    };
    let destination_width = destination.width() as usize;
    let destination_height = destination.height() as usize;
    let source_width = source.width() as usize;
    let source_height = source.height() as usize;
    let destination_pixels = destination_width.checked_mul(destination_height)?;
    if source_width.checked_mul(source_height)? != mask.len() {
        return None;
    }
    let value_count = destination_pixels.checked_mul(lane_count)?;
    let mut values = Vec::new();
    values.try_reserve_exact(value_count).ok()?;
    let destination_lanes = destination.hidden_pixel_lanes();
    let source_lanes = source.hidden_pixel_lanes();
    if destination_lanes.is_some_and(|lanes| {
        lanes.lane_count() != lane_count || lanes.pixel_count() != destination_pixels
    }) || source_lanes
        .is_some_and(|lanes| lanes.lane_count() != lane_count || lanes.pixel_count() != mask.len())
    {
        return None;
    }

    for destination_index in 0..destination_pixels {
        let destination_x = destination_index % destination_width;
        let destination_y = destination_index / destination_width;
        let source_coordinates = destination_x
            .checked_sub(x as usize)
            .zip(destination_y.checked_sub(y as usize))
            .filter(|(source_x, source_y)| {
                *source_x < source_width
                    && *source_y < source_height
                    && mask[*source_y * source_width + *source_x] != 0
            });
        let (image, retained, pixel_index) = if let Some((source_x, source_y)) = source_coordinates
        {
            let source_index = source_y * source_width + source_x;
            (source, source_lanes, source_index)
        } else {
            (destination, destination_lanes, destination_index)
        };
        let lanes = hidden_lanes_for(image, mode, retained, pixel_index, lane_count)?;
        values.extend_from_slice(&lanes[..lane_count]);
    }
    HiddenPixelLanes::per_pixel(lane_count, destination_pixels, values).map(Some)
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
fn hidden_lanes_for(
    image: &DynamicImage,
    mode: Mode,
    retained: Option<&HiddenPixelLanes>,
    pixel_index: usize,
    lane_count: usize,
) -> Option<[u8; 2]> {
    if let Some(retained) = retained {
        let lanes = retained.for_pixel(pixel_index)?;
        let mut values = [0; 2];
        values[..lane_count].copy_from_slice(lanes);
        return Some(values);
    }
    let carrier_width = mode.byte_carrier_pixel_width()?;
    let offset = pixel_index.checked_mul(carrier_width)?;
    let pixel = image
        .as_bytes()
        .get(offset..offset.checked_add(carrier_width)?)?;
    Some(match carrier_width {
        2 => [pixel[0], pixel[0]],
        3 => [u8::MAX, 0],
        _ => return None,
    })
}
