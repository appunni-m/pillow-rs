#[cfg(test)]
mod native_l_rank_filter_second_min_tests {
    use super::super::{rank_filter_bytes_3x3, rank_filter_bytes_small};

    #[test]
    fn rank_one_second_min_matches_selection_for_ties_borders_and_tails() {
        for (width, height) in [(1usize, 1usize), (1, 11), (2, 3), (17, 9), (33, 35)] {
            let raw = (0..width * height)
                .map(|index| ((index * 37 + index / width * 11 + 3) % 5) as u8)
                .collect::<Vec<_>>();
            let mut actual = vec![0u8; raw.len()];
            let mut selected = vec![0u8; raw.len()];

            rank_filter_bytes_3x3(&raw, &mut actual, width, height, 1, 1);
            rank_filter_bytes_small(&raw, &mut selected, width as i32, height as i32, 1, 1, 9, 1);

            assert_eq!(actual, selected, "native L {width}x{height}");
        }
    }
}
