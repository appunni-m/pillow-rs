use super::Image;

#[test]
fn percent_150_adjustment_matches_signed_truncation_for_every_byte_difference() {
    for difference in -255..=255 {
        assert_eq!(
            Image::pil_unsharp_adjustment_150(difference),
            difference * 150 / 100,
            "difference={difference}"
        );
    }
}
