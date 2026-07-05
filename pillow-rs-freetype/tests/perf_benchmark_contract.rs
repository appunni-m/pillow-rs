//! Contract tests for the FreeType performance benchmark matrix.

#![allow(unused_crate_dependencies)]
#![allow(clippy::expect_used)]

use std::collections::HashSet;
use std::path::Path;

use serde_json::Value;

const MATRIX: &str = include_str!("data/perf_operation_matrix.json");

fn matrix() -> Value {
    serde_json::from_str(MATRIX).expect("perf operation matrix must be valid JSON")
}

#[test]
fn perf_matrix_rows_have_auditable_timing_contracts() {
    let data = matrix();
    let rows = data["rows"]
        .as_array()
        .expect("perf operation matrix must have rows");
    assert!(!rows.is_empty(), "perf matrix must not be empty");

    let mut ids = HashSet::new();
    for row in rows {
        let id = row["id"].as_str().expect("row must have string id");
        assert!(ids.insert(id), "duplicate perf row id {id}");
        assert!(
            row["operation"]
                .as_str()
                .is_some_and(|value| !value.is_empty()),
            "{id} must declare an operation"
        );
        assert!(
            row["font"].as_str().is_some_and(|value| !value.is_empty()),
            "{id} must declare a font fixture"
        );
        let iterations = row["iterations"]
            .as_u64()
            .expect("row iterations must be a positive integer");
        assert!(iterations > 0, "{id} must have positive iterations");
        let weight = row["weight"]
            .as_f64()
            .expect("row weight must be a positive number");
        assert!(weight > 0.0, "{id} must have positive row weight");
        assert!(
            row["timing_boundary"]
                .as_str()
                .is_some_and(|value| value.len() >= 24),
            "{id} must document its timing boundary"
        );
        let trust = row["comparison_trust"]
            .as_str()
            .expect("row must declare comparison_trust");
        assert!(
            matches!(trust, "timing_only" | "exact_sha256"),
            "{id} has unsupported comparison_trust {trust}"
        );

        let font = row["font"].as_str().expect("checked above");
        assert!(
            Path::new(env!("CARGO_MANIFEST_DIR")).join(font).exists(),
            "{id} references missing font fixture {font}"
        );
    }
}

#[test]
fn workload_profiles_cover_every_perf_row_with_positive_weights() {
    let data = matrix();
    let rows = data["rows"]
        .as_array()
        .expect("perf operation matrix must have rows");
    let row_ids: HashSet<&str> = rows
        .iter()
        .map(|row| row["id"].as_str().expect("row must have id"))
        .collect();

    let profiles = data["workload_profiles"]
        .as_object()
        .expect("perf matrix must define workload_profiles");
    assert!(
        profiles.contains_key("default"),
        "perf matrix must define a default workload profile"
    );

    for (profile_name, profile) in profiles {
        assert!(
            profile["description"]
                .as_str()
                .is_some_and(|value| value.len() >= 16),
            "{profile_name} must document profile intent"
        );
        let weights = profile["weights"]
            .as_object()
            .expect("profile must define weights");
        let weight_ids: HashSet<&str> = weights.keys().map(String::as_str).collect();
        assert_eq!(
            weight_ids, row_ids,
            "{profile_name} must assign a weight to every perf row and no unknown row"
        );
        for (row_id, weight) in weights {
            let weight = weight.as_f64().expect("profile weight must be numeric");
            assert!(
                weight > 0.0,
                "{profile_name}/{row_id} must have positive weight"
            );
        }
    }
}
