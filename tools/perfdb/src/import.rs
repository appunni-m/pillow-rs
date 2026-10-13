use std::{collections::BTreeMap, fs, path::Path};

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::model::{
    Observation, WorkloadDefinition, array_size, clean_optional_text, parse_bool, parse_f64,
    parse_i64, signature, split_context, string, workload_signature,
};

#[derive(Debug)]
pub struct ArtifactImport {
    pub sha256: String,
    pub source_path: String,
    pub format: String,
    pub source_kind: String,
    pub schema_name: Option<String>,
    pub repository: Option<String>,
    pub revision: Option<String>,
    pub measured_at: Option<String>,
    pub run_id: Option<String>,
    pub metadata: Value,
    pub rows: Vec<Observation>,
}

pub fn read_artifact(path: &Path) -> Result<ArtifactImport> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let sha256 = format!("{:x}", Sha256::digest(&bytes));
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "json" => parse_json(path, &bytes, sha256),
        "csv" => parse_csv(path, &bytes, sha256),
        _ => bail!("unsupported artifact extension for {}", path.display()),
    }
}

fn parse_json(path: &Path, bytes: &[u8], sha256: String) -> Result<ArtifactImport> {
    let root: Value = serde_json::from_slice(bytes)
        .with_context(|| format!("parse JSON artifact {}", path.display()))?;
    let schema_name = string(&root, "schema");
    let repository = string(&root, "repository");
    let revision = string(&root, "revision");
    let measured_at = string(&root, "measured_at");
    let run_id = string(&root, "run_id");
    let mut metadata = root.clone();
    if let Some(object) = metadata.as_object_mut() {
        for field in ["rows", "workloads"] {
            object.remove(field);
        }
    }

    let (source_kind, rows) = match schema_name.as_deref() {
        Some("public-docs/benchmark-snapshot@1") | Some("public-docs/benchmark-snapshot@2") => {
            ("benchmark-snapshot", import_snapshot(&root)?)
        }
        Some("migration-parity/benchmark-result@1") => {
            ("benchmark-result", import_benchmark_result(&root)?)
        }
        _ => (
            "raw-json",
            vec![Observation {
                source_kind: "raw-json".into(),
                record_kind: "artifact".into(),
                raw: root.clone(),
                ..Observation::default()
            }],
        ),
    };

    Ok(ArtifactImport {
        sha256,
        source_path: path.display().to_string(),
        format: "json".into(),
        source_kind: source_kind.into(),
        schema_name,
        repository,
        revision,
        measured_at,
        run_id,
        metadata,
        rows,
    })
}

fn import_snapshot(root: &Value) -> Result<Vec<Observation>> {
    let repository = string(root, "repository");
    let revision = string(root, "revision");
    let measured_at = string(root, "measured_at");
    let run_id = string(root, "run_id");
    let machine_map = machines(root);
    let source_rows = root
        .get("rows")
        .and_then(Value::as_array)
        .context("benchmark snapshot has no rows array")?;

    let mut imported = Vec::with_capacity(source_rows.len());
    for row in source_rows {
        let machine_id = string(row, "machine_id");
        let comparison_group = string(row, "comparison_group");
        let machine = machine_id
            .as_ref()
            .and_then(|id| machine_map.get(id))
            .cloned()
            .or_else(|| root.get("machine").cloned())
            .unwrap_or(Value::Null);
        let machine_signature = (!machine.is_null()).then(|| signature(&machine));
        let (cohort_run, row_measured_at) = cohort_details(
            root,
            machine_id.as_deref(),
            comparison_group.as_deref(),
            string(row, "subject").as_deref(),
        );
        let cohort_run = cohort_run.or_else(|| run_id.clone());
        let row_measured_at = row_measured_at.or_else(|| measured_at.clone());

        let policy = row.get("policy").cloned();
        let context = row.get("context").cloned();
        let (context_mode, context_width, context_height, context_chain) = context
            .as_ref()
            .map(split_context)
            .unwrap_or((None, None, None, None));
        let workload = string(row, "workload");
        let mode = string(row, "mode").or(context_mode);
        let width = parse_i64(row, "width").or(context_width);
        let height = parse_i64(row, "height").or(context_height);
        let chain_length = parse_i64(row, "chain_length").or(context_chain);
        let boundary = policy.as_ref().and_then(|p| string(p, "boundary"));
        let sample_unit = string(row, "sample_unit");
        let signature = workload_signature(&WorkloadDefinition {
            workload: &workload,
            mode: &mode,
            width,
            height,
            chain_length,
            boundary: &boundary,
            sample_unit: &sample_unit,
            policy: &policy,
            context: &context,
            input_hashes: None,
        });
        let subject = string(row, "subject");
        let scope = scope_from_workload(workload.as_deref());
        let mut raw = Map::new();
        raw.insert("record".into(), row.clone());
        raw.insert("machine".into(), machine);
        raw.insert(
            "snapshot".into(),
            json!({
                "repository": repository,
                "revision": revision,
                "measured_at": row_measured_at,
                "run_id": run_id,
                "cohort_run_id": cohort_run,
            }),
        );
        imported.push(Observation {
            source_kind: "benchmark-snapshot".into(),
            record_kind: "benchmark-case".into(),
            raw: Value::Object(raw),
            workload,
            operation: None,
            scope,
            operation_class: context
                .as_ref()
                .and_then(|value| string(value, "operation_class")),
            subject: subject.clone(),
            profile: subject,
            baseline: None,
            comparison_group,
            run_id: cohort_run,
            revision: revision.clone(),
            measured_at: row_measured_at,
            runner_id: machine_id,
            runner_signature: machine_signature,
            workload_signature: signature,
            mode,
            width,
            height,
            chain_length,
            boundary,
            sample_unit,
            sample_count: parse_i64(row, "sample_count"),
            median_us: parse_f64(row, "median_us"),
            p95_us: parse_f64(row, "p95_us"),
            throughput_ops_s: None,
            verified_speedup_vs_pillow: None,
            verified_speedup_vs_pillow_simd: None,
            verified_gpu_latency_speedup_vs_simd: None,
            gpu_sustained_throughput_status: None,
            status: string(row, "status"),
            correctness: string(row, "correctness"),
            evidence_status: None,
            requested_backend: string(row, "requested_backend"),
            actual_backend: string(row, "actual_backend"),
            terminal_complete: parse_bool(row, "terminal_complete"),
            fallback_reasons: row.get("fallback_reasons").map(Value::to_string),
            policy,
            context,
            resource: None,
        });
    }
    Ok(imported)
}

fn machines(root: &Value) -> BTreeMap<String, Value> {
    let mut result = BTreeMap::new();
    if let Some(entries) = root.get("machines").and_then(Value::as_array) {
        for machine in entries {
            if let Some(id) = string(machine, "id") {
                result.insert(id, machine.clone());
            }
        }
    }
    if let Some(machine) = root.get("machine")
        && let Some(id) = string(machine, "id")
    {
        result.insert(id, machine.clone());
    }
    result
}

fn cohort_details(
    root: &Value,
    machine_id: Option<&str>,
    comparison_group: Option<&str>,
    subject: Option<&str>,
) -> (Option<String>, Option<String>) {
    let Some(entries) = root.get("cohorts").and_then(Value::as_array) else {
        return (None, None);
    };
    let root_machine = root
        .get("machine")
        .and_then(|machine| string(machine, "id"));
    entries
        .iter()
        .find(|cohort| {
            let cohort_machine = string(cohort, "machine_id").or_else(|| root_machine.clone());
            let cohort_id = string(cohort, "id");
            cohort_machine.as_deref() == machine_id
                && (cohort_id.as_deref() == comparison_group || cohort_id.as_deref() == subject)
        })
        .map(|cohort| (string(cohort, "run_id"), string(cohort, "measured_at")))
        .unwrap_or((None, None))
}

fn import_benchmark_result(root: &Value) -> Result<Vec<Observation>> {
    let identity = root.get("identity").cloned().unwrap_or(Value::Null);
    let run_id = string(&identity, "run_id");
    let revision = identity
        .get("targets")
        .and_then(Value::as_array)
        .and_then(|targets| targets.iter().find_map(|target| string(target, "revision")))
        .or_else(|| string(root, "revision"));
    let measured_at = string(&identity, "started_at");
    let environment = root.get("environment").cloned().unwrap_or(Value::Null);
    let runner_id = string(&environment, "runner")
        .or_else(|| string(&environment, "host"))
        .or_else(|| {
            Some(format!(
                "{}-{}",
                string(&environment, "os")?,
                string(&environment, "architecture")?
            ))
        });
    let runner_signature = (!environment.is_null()).then(|| signature(&environment));
    let input_hashes = identity.get("inputs").map(|inputs| {
        Value::Array(
            inputs
                .as_array()
                .into_iter()
                .flatten()
                .map(|input| {
                    json!({
                        "path": string(input, "path"),
                        "sha256": string(input, "sha256"),
                    })
                })
                .collect(),
        )
    });
    let workloads = root
        .get("workloads")
        .and_then(Value::as_array)
        .context("benchmark result has no workloads array")?;
    let mut imported = Vec::new();

    for workload_row in workloads {
        let workload = string(workload_row, "workload_id");
        let policy = workload_row.get("measurement_policy").cloned();
        let context = workload_row.get("context").cloned();
        let (mode, width, height, chain_length) =
            context.as_ref().map(split_context).unwrap_or_else(|| {
                let (width, height) = array_size(workload_row, "size");
                (string(workload_row, "mode"), width, height, None)
            });
        let boundary = policy.as_ref().and_then(|p| string(p, "boundary"));
        let sample_unit = policy.as_ref().and_then(|p| string(p, "sample_unit"));
        let workload_sig = workload_signature(&WorkloadDefinition {
            workload: &workload,
            mode: &mode,
            width,
            height,
            chain_length,
            boundary: &boundary,
            sample_unit: &sample_unit,
            policy: &policy,
            context: &context,
            input_hashes: input_hashes.as_ref(),
        });
        let correctness_value = workload_row
            .get("correctness")
            .cloned()
            .unwrap_or(Value::Null);
        let correctness = match (
            string(&correctness_value, "gate"),
            string(&correctness_value, "outcome"),
        ) {
            (Some(gate), Some(outcome)) => Some(format!("{gate}: {outcome}")),
            _ => correctness_value
                .as_str()
                .map(ToOwned::to_owned)
                .filter(|s| !s.is_empty()),
        };
        let operation = workload_row
            .get("requirements")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .or_else(|| workload.clone());

        let subjects = workload_row
            .get("subjects")
            .and_then(Value::as_array)
            .context("benchmark result workload has no subjects array")?;
        for subject in subjects {
            let subject_id = string(subject, "id").unwrap_or_else(|| "unknown".into());
            let measurements = subject
                .get("measurements")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let latency = measurements
                .iter()
                .find(|item| string(item, "metric").as_deref() == Some("latency"));
            let throughput = measurements
                .iter()
                .find(|item| string(item, "metric").as_deref() == Some("throughput"));
            let latency_scale = latency
                .and_then(|m| string(m, "unit"))
                .map(|unit| unit_scale_to_us(&unit))
                .unwrap_or(1.0);
            let throughput_scale = throughput
                .and_then(|m| string(m, "unit"))
                .map(|unit| unit_scale_to_ops_s(&unit))
                .unwrap_or(1.0);
            let median_us = latency
                .and_then(|m| m.get("statistics"))
                .and_then(|stats| parse_f64(stats, "median"))
                .map(|value| value * latency_scale);
            let p95_us = latency
                .and_then(|m| m.get("statistics"))
                .and_then(|stats| parse_f64(stats, "p95"))
                .map(|value| value * latency_scale);
            let sample_count = latency.and_then(|m| parse_i64(m, "sample_count"));
            let throughput_ops_s = throughput
                .and_then(|m| m.get("statistics"))
                .and_then(|stats| parse_f64(stats, "median"))
                .map(|value| value * throughput_scale);
            let execution = subject.get("execution").cloned().unwrap_or(Value::Null);
            let mut raw = Map::new();
            raw.insert("workload".into(), workload_row.clone());
            raw.insert("subject".into(), subject.clone());
            raw.insert("identity".into(), identity.clone());
            raw.insert("environment".into(), environment.clone());
            let backend = string(&execution, "actual_backend");
            imported.push(Observation {
                source_kind: "benchmark-result".into(),
                record_kind: "benchmark-subject".into(),
                raw: Value::Object(raw),
                workload: workload.clone(),
                operation: operation.clone(),
                scope: scope_from_workload(workload.as_deref()),
                operation_class: context
                    .as_ref()
                    .and_then(|value| string(value, "operation_class")),
                subject: Some(subject_id.clone()),
                profile: Some(subject_id.clone()),
                baseline: (string(subject, "kind").as_deref() == Some("oracle"))
                    .then(|| "Pillow".into()),
                comparison_group: Some("run".into()),
                run_id: run_id.clone(),
                revision: revision.clone(),
                measured_at: measured_at.clone(),
                runner_id: runner_id.clone(),
                runner_signature: runner_signature.clone(),
                workload_signature: workload_sig.clone(),
                mode: mode.clone(),
                width,
                height,
                chain_length,
                boundary: boundary.clone(),
                sample_unit: sample_unit.clone(),
                sample_count,
                median_us,
                p95_us,
                throughput_ops_s,
                verified_speedup_vs_pillow: None,
                verified_speedup_vs_pillow_simd: None,
                verified_gpu_latency_speedup_vs_simd: None,
                gpu_sustained_throughput_status: None,
                status: string(subject, "status"),
                correctness: correctness.clone(),
                evidence_status: None,
                requested_backend: string(&execution, "requested_backend"),
                actual_backend: backend,
                terminal_complete: parse_bool(&execution, "terminal_complete"),
                fallback_reasons: execution
                    .get("fallback_reason_counts")
                    .map(Value::to_string),
                policy: policy.clone(),
                context: context.clone(),
                resource: Some(execution),
            });
        }
    }
    Ok(imported)
}

fn unit_scale_to_us(unit: &str) -> f64 {
    match unit.to_ascii_lowercase().as_str() {
        "ns" | "nanosecond" | "nanoseconds" => 0.001,
        "us" | "µs" | "microsecond" | "microseconds" => 1.0,
        "ms" | "millisecond" | "milliseconds" => 1_000.0,
        "s" | "second" | "seconds" => 1_000_000.0,
        _ => 1.0,
    }
}

fn unit_scale_to_ops_s(unit: &str) -> f64 {
    match unit.to_ascii_lowercase().as_str() {
        "operations_per_millisecond" | "ops/ms" => 1_000.0,
        "operations_per_second" | "ops/s" | "operations/second" => 1.0,
        "operations_per_minute" | "ops/min" => 1.0 / 60.0,
        _ => 1.0,
    }
}

fn parse_csv(path: &Path, bytes: &[u8], sha256: String) -> Result<ArtifactImport> {
    let mut reader = csv::ReaderBuilder::new().flexible(true).from_reader(bytes);
    let headers = reader.headers()?.clone();
    let names = headers.iter().collect::<Vec<_>>();
    let (source_kind, record_kind) = classify_csv(&names);
    let mut rows = Vec::new();
    for result in reader.deserialize::<BTreeMap<String, String>>() {
        let row = result.with_context(|| format!("read CSV row from {}", path.display()))?;
        let raw = serde_json::to_value(&row)?;
        rows.push(csv_observation(source_kind, record_kind, row, raw));
    }
    Ok(ArtifactImport {
        sha256,
        source_path: path.display().to_string(),
        format: "csv".into(),
        source_kind: source_kind.into(),
        schema_name: None,
        repository: None,
        revision: rows.iter().find_map(|row| row.revision.clone()),
        measured_at: rows.iter().find_map(|row| row.measured_at.clone()),
        run_id: None,
        metadata: json!({"headers": names, "row_count": rows.len()}),
        rows,
    })
}

fn classify_csv(headers: &[&str]) -> (&'static str, &'static str) {
    let has = |field: &str| headers.contains(&field);
    if has("source_json_sha256") && has("verified_speedup_vs_pillow") {
        ("case-matrix", "case-comparison")
    } else if has("api_path") && has("benchmark_workload_ids") {
        ("operation-matrix", "operation-summary")
    } else if has("experiment") && has("workload_id") && has("actual_backend") {
        ("local-run-ledger", "local-run")
    } else {
        ("raw-csv", "csv-row")
    }
}

fn csv_observation(
    source_kind: &str,
    record_kind: &str,
    row: BTreeMap<String, String>,
    raw: Value,
) -> Observation {
    let text = |key: &str| {
        row.get(key)
            .cloned()
            .and_then(|value| clean_optional_text(Some(value)))
    };
    let number = |key: &str| row.get(key).and_then(|v| v.parse::<f64>().ok());
    let integer = |key: &str| row.get(key).and_then(|v| v.parse::<i64>().ok());
    let boolean = |key: &str| {
        row.get(key).and_then(|v| match v.as_str() {
            "true" | "True" | "1" => Some(true),
            "false" | "False" | "0" => Some(false),
            _ => None,
        })
    };
    let workload = if source_kind == "local-run-ledger" {
        text("workload_id")
    } else {
        text("workload")
    };
    let operation = if source_kind == "case-matrix" {
        text("public_apis").or_else(|| text("operation"))
    } else {
        text("operation").or_else(|| text("api_path"))
    };
    let scope = text("scope").or_else(|| scope_from_workload(workload.as_deref()));
    let operation_class = text("operation_class").or_else(|| {
        (source_kind == "operation-matrix")
            .then(|| text("classification"))
            .flatten()
    });
    let profile = text("profile");
    let subject = profile.clone();
    let mode = text("mode");
    let width = integer("width");
    let height = integer("height");
    let chain_length = integer("chain_length");
    let boundary = text("measurement_boundary");
    let sample_unit = text("sample_unit");
    let policy = if source_kind == "case-matrix" {
        Some(json!({
            "boundary": boundary,
            "warmup_iterations": integer("warmup_iterations"),
            "measurement_iterations": integer("measurement_iterations"),
            "samples": integer("samples"),
            "concurrency": integer("concurrency"),
            "sample_unit": sample_unit,
            "cache_state": text("cache_state"),
            "correctness_gate": text("correctness_gate"),
        }))
    } else if source_kind == "local-run-ledger" {
        Some(json!({
            "boundary": boundary,
            "step_ids": text("step_ids"),
            "cache_state": text("cache_state"),
            "warmup_iterations": integer("warmup_iterations"),
            "measurement_iterations": integer("measurement_iterations"),
            "sample_count": integer("sample_count"),
        }))
    } else {
        None
    };
    let context = if source_kind == "case-matrix" {
        Some(json!({
            "mode": mode,
            "width": width,
            "height": height,
            "chain_length": chain_length,
            "cache_state": text("cache_state"),
            "build_profile": text("build_profile"),
        }))
    } else if source_kind == "local-run-ledger" {
        Some(json!({
            "mode": mode,
            "width": width,
            "height": height,
            "cache_state": text("cache_state"),
            "candidate_disposition": text("candidate_disposition"),
        }))
    } else {
        None
    };
    let runner_id = if source_kind == "case-matrix" {
        text("runner")
    } else {
        text("host")
    };
    let runner_signature = runner_id.as_ref().map(|runner| {
        if source_kind == "case-matrix" {
            format!("unresolved-runner:{runner}")
        } else {
            signature(&json!({
                "host": runner,
                "os": text("os"),
                "architecture": text("architecture"),
            }))
        }
    });
    let measured_at = text("measured_at")
        .or_else(|| text("started_at"))
        .or_else(|| text("full_snapshot_measured_at"));
    let revision = text("source_revision")
        .or_else(|| text("full_snapshot_revision"))
        .or_else(|| text("revision"));
    let workload_signature = workload_signature(&WorkloadDefinition {
        workload: &workload,
        mode: &mode,
        width,
        height,
        chain_length,
        boundary: &boundary,
        sample_unit: &sample_unit,
        policy: &policy,
        context: &context,
        input_hashes: None,
    });
    let fallback_reasons = text("fallback_reasons").or_else(|| text("fallback_reason_counts"));
    let correctness = text("correctness").or_else(|| text("parity_status"));
    let evidence_status = text("evidence_status").or_else(|| text("profile_status"));
    let baseline = text("baseline");
    let resource = if source_kind == "local-run-ledger" {
        let telemetry_fields = [
            "actual_backend_counts",
            "fallback_reason_counts",
            "operation_count",
            "dispatch_count",
            "upload_bytes",
            "readback_bytes",
            "parameter_bytes",
            "host_allocation_count",
            "host_allocated_bytes",
            "peak_live_host_bytes",
            "mode_conversion_count",
            "fused_operation_count",
            "gpu_sustained_throughput",
        ];
        Some(Value::Object(
            telemetry_fields
                .iter()
                .filter_map(|field| {
                    raw.get(*field)
                        .map(|value| ((*field).into(), value.clone()))
                })
                .collect(),
        ))
    } else {
        None
    };
    let record_kind = if source_kind == "operation-matrix" {
        "operation-summary"
    } else {
        record_kind
    };

    Observation {
        source_kind: source_kind.into(),
        record_kind: record_kind.into(),
        raw,
        workload,
        operation,
        scope,
        operation_class,
        subject,
        profile,
        baseline,
        comparison_group: text("comparison_group"),
        run_id: text("run_id"),
        revision,
        measured_at,
        runner_id,
        runner_signature,
        workload_signature,
        mode,
        width,
        height,
        chain_length,
        boundary,
        sample_unit,
        sample_count: integer("sample_count"),
        median_us: number("median_us"),
        p95_us: number("p95_us"),
        throughput_ops_s: number("throughput_ops_s"),
        verified_speedup_vs_pillow: number("verified_speedup_vs_pillow"),
        verified_speedup_vs_pillow_simd: number("verified_speedup_vs_pillow_simd"),
        verified_gpu_latency_speedup_vs_simd: number("verified_gpu_latency_speedup_vs_simd"),
        gpu_sustained_throughput_status: text("gpu_sustained_throughput_status")
            .or_else(|| text("gpu_sustained_throughput")),
        status: text("status"),
        correctness,
        evidence_status,
        requested_backend: text("requested_backend"),
        actual_backend: text("actual_backend"),
        terminal_complete: boolean("terminal_complete"),
        fallback_reasons,
        policy,
        context,
        resource,
    }
}

fn scope_from_workload(workload: Option<&str>) -> Option<String> {
    let workload = workload?;
    let scope = if workload.starts_with("pipeline-chain.") {
        "composed_pipeline"
    } else if workload.starts_with("pipeline-op.") || workload.starts_with("pil-image-") {
        "individual_operation"
    } else {
        return None;
    };
    Some(scope.into())
}
