// ============================================================================
// AS PER DESIGN — DO NOT REMOVE:
//   InfallibleExt provides a `.because(reason)` method that replaces bare
//   `.unwrap()` and `.expect()` for truly infallible operations. The `reason`
//   parameter documents WHY the operation cannot fail — this is required for
//   code review and survives into git blame.
//
//   The workspace lints `unwrap_used` and `expect_used` are both "deny".
//   This trait is the ONLY way to call expect() outside of tests.
//
//   Usage:
//     let img = RgbaImage::from_raw(w, h, data)
//         .because("CheckedDims guarantees buf.len() == w*h*channels");
// ============================================================================

/// AS PER DESIGN — DO NOT REMOVE:
/// Extension trait for Option and Result types that provides a documented
/// unwrap alternative. The `reason` string documents the invariant that
/// guarantees the value is present — this survives in git blame unlike
/// a bare expect() message.
pub trait InfallibleExt {
    type Output;

    /// Unwrap the value, documenting WHY it's infallible.
    /// AS PER DESIGN: `reason` is required. It must explain the invariant
    /// that guarantees the value exists, not just what failed.
    #[track_caller]
    fn because(self, reason: &'static str) -> Self::Output;
}

impl<T> InfallibleExt for Option<T> {
    type Output = T;

    #[track_caller]
    fn because(self, reason: &'static str) -> T {
        match self {
            Some(v) => v,
            None => panic!(
                "invariant violated: {}\n\
                 This panic indicates a bug, not user error. \
                 The invariant documented at this call site did not hold.",
                reason
            ),
        }
    }
}

impl<T, E: std::fmt::Debug> InfallibleExt for Result<T, E> {
    type Output = T;

    #[track_caller]
    fn because(self, reason: &'static str) -> T {
        match self {
            Ok(v) => v,
            Err(e) => panic!(
                "invariant violated: {}\n\
                 Error: {:?}\n\
                 This panic indicates a bug, not user error. \
                 The invariant documented at this call site did not hold.",
                reason, e
            ),
        }
    }
}

// AS PER DESIGN — DO NOT REMOVE: Tests validate behavior.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn option_because_success() {
        let x = Some(42);
        assert_eq!(x.because("test always has a value"), 42);
    }

    #[test]
    #[should_panic(expected = "invariant violated")]
    fn option_because_panics_with_message() {
        let x: Option<i32> = None;
        x.because("test should always have a value");
    }

    #[test]
    fn result_because_success() {
        let x: Result<i32, &str> = Ok(42);
        assert_eq!(x.because("test result is always Ok"), 42);
    }

    #[test]
    #[should_panic(expected = "invariant violated")]
    fn result_because_panics_with_message() {
        let x: Result<i32, &str> = Err("something failed");
        x.because("test result should always be Ok");
    }
}
