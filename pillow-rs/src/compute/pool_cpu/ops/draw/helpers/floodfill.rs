//! Shared callback-driven flood-fill traversal for the Draw family.

use crate::draw::FloodFillAccess;

pub(crate) fn run<A: FloodFillAccess>(
    access: &mut A,
    seed: A::Point,
    value: &A::Pixel,
    border: Option<&A::Pixel>,
    threshold: &A::Threshold,
) -> Result<(), A::Error> {
    let Some(background) = fill_seed(access, &seed, value, threshold)? else {
        return Ok(());
    };
    let mut edge = access.new_frontier()?;
    access.frontier_insert(&mut edge, seed)?;
    let mut full_edge = access.new_frontier()?;
    while !access.frontier_is_empty(&edge)? {
        let mut next_edge = access.new_frontier()?;
        fill_frontier(
            access,
            &edge,
            &mut full_edge,
            &mut next_edge,
            value,
            border,
            &background,
            threshold,
        )?;
        // Pillow retains only the just-processed edge for the next pass.
        full_edge = edge;
        edge = next_edge;
    }
    Ok(())
}

fn fill_seed<A: FloodFillAccess>(
    access: &mut A,
    seed: &A::Point,
    value: &A::Pixel,
    threshold: &A::Threshold,
) -> Result<Option<A::Pixel>, A::Error> {
    let background = match access.get_pixel(seed) {
        Ok(background) => background,
        Err(error) if access.is_swallowed_error(&error) => return Ok(None),
        Err(error) => return Err(error),
    };
    let should_fill = match color_diff_is_within(access, value, &background, threshold) {
        Ok(should_fill) => should_fill,
        Err(error) if access.is_swallowed_error(&error) => return Ok(None),
        Err(error) => return Err(error),
    };
    if !should_fill {
        return Ok(None);
    }
    match access.set_pixel(seed, value) {
        Ok(()) => Ok(Some(background)),
        Err(error) if access.is_swallowed_error(&error) => Ok(None),
        Err(error) => Err(error),
    }
}

fn fill_frontier<A: FloodFillAccess>(
    access: &mut A,
    edge: &A::Frontier,
    full_edge: &mut A::Frontier,
    next_edge: &mut A::Frontier,
    value: &A::Pixel,
    border: Option<&A::Pixel>,
    background: &A::Pixel,
    threshold: &A::Threshold,
) -> Result<(), A::Error> {
    let mut points = access.frontier_iter(edge)?;
    while let Some(point) = access.frontier_next(&mut points)? {
        fill_neighbors(
            access, &point, full_edge, next_edge, value, border, background, threshold,
        )?;
    }
    Ok(())
}

fn fill_neighbors<A: FloodFillAccess>(
    access: &mut A,
    point: &A::Point,
    full_edge: &mut A::Frontier,
    next_edge: &mut A::Frontier,
    value: &A::Pixel,
    border: Option<&A::Pixel>,
    background: &A::Pixel,
    threshold: &A::Threshold,
) -> Result<(), A::Error> {
    for neighbor in access.neighbors(point)? {
        // Pillow checks set membership before its short-circuiting negatives.
        if access.frontier_contains(full_edge, &neighbor)?
            || !access.point_is_nonnegative(&neighbor)?
        {
            continue;
        }
        let pixel = match access.get_pixel(&neighbor) {
            Ok(pixel) => pixel,
            Err(error) if access.is_swallowed_error(&error) => continue,
            Err(error) => return Err(error),
        };
        access.frontier_insert(full_edge, neighbor.clone())?;
        if should_fill_neighbor(access, &pixel, value, border, background, threshold)? {
            access.set_pixel(&neighbor, value)?;
            access.frontier_insert(next_edge, neighbor)?;
        }
    }
    Ok(())
}

fn should_fill_neighbor<A: FloodFillAccess>(
    access: &mut A,
    pixel: &A::Pixel,
    value: &A::Pixel,
    border: Option<&A::Pixel>,
    background: &A::Pixel,
    threshold: &A::Threshold,
) -> Result<bool, A::Error> {
    match border {
        Some(border) => access.differs_from_fill_and_border(pixel, value, border),
        None => color_diff_is_within(access, pixel, background, threshold),
    }
}

fn color_diff_is_within<A: FloodFillAccess>(
    access: &mut A,
    first: &A::Pixel,
    second: &A::Pixel,
    threshold: &A::Threshold,
) -> Result<bool, A::Error> {
    let (first, second) = access.color_vectors(first, second)?;
    let length = access.color_vector_len(&second)?;
    let mut sum = access.color_sum_start()?;
    // Pillow iterates range(len(second)), even when first has fewer samples.
    for index in 0..length {
        let difference = access.color_difference_at(&first, &second, index)?;
        sum = access.add_color_difference(sum, difference)?;
    }
    access.color_sum_is_within_threshold(&sum, threshold)
}
