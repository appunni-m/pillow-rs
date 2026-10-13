use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};
use duckdb::{Connection, params};

use crate::store::short;

pub fn status(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare(
        "SELECT source_kind, count(*) AS artifacts, sum(row_count) AS rows,
                max(measured_at) AS latest
         FROM artifacts
         GROUP BY source_kind
         ORDER BY source_kind",
    )?;
    let mut rows = statement.query([])?;
    println!("\nArtifacts by category");
    println!("{:<24} {:>9} {:>10}  LATEST", "SOURCE", "ARTIFACTS", "ROWS");
    while let Some(row) = rows.next()? {
        let source: String = row.get(0)?;
        let artifacts: i64 = row.get(1)?;
        let count: Option<i64> = row.get(2)?;
        let latest: Option<String> = row.get(3)?;
        println!(
            "{source:<24} {artifacts:>9} {:>10}  {}",
            count.unwrap_or_default(),
            latest.unwrap_or_else(|| "-".into())
        );
    }
    let observations: i64 =
        connection.query_row("SELECT count(*) FROM observations", [], |row| row.get(0))?;
    let links: i64 =
        connection.query_row("SELECT count(*) FROM artifact_rows", [], |row| row.get(0))?;
    println!("\nUnique observations: {observations}; artifact row links: {links}");

    let mut statement = connection.prepare(
        "SELECT evidence_status, count(*) FROM observations
         WHERE source_kind = 'case-matrix'
         GROUP BY evidence_status ORDER BY evidence_status NULLS FIRST",
    )?;
    let mut rows = statement.query([])?;
    println!("\nPer-case evidence status (all imported case matrices)");
    while let Some(row) = rows.next()? {
        let status: Option<String> = row.get(0)?;
        let count: i64 = row.get(1)?;
        println!(
            "  {:<32} {}",
            status.unwrap_or_else(|| "<none>".into()),
            count
        );
    }

    let mut statement = connection.prepare(
        "SELECT scope, profile, count(*)
         FROM observations
         WHERE source_kind = 'case-matrix'
         GROUP BY scope, profile
         ORDER BY scope NULLS FIRST, profile NULLS FIRST",
    )?;
    let mut rows = statement.query([])?;
    println!("\nPer-case rows by pipeline/operation scope and profile");
    while let Some(row) = rows.next()? {
        let scope: Option<String> = row.get(0)?;
        let profile: Option<String> = row.get(1)?;
        let count: i64 = row.get(2)?;
        println!(
            "  {:<24} {:<20} {}",
            scope.unwrap_or_else(|| "<none>".into()),
            profile.unwrap_or_else(|| "<none>".into()),
            count
        );
    }

    let mut statement = connection.prepare(
        "SELECT gpu_sustained_throughput_status, count(*)
         FROM observations
         WHERE source_kind = 'case-matrix' AND profile = 'python-gpu'
           AND gpu_sustained_throughput_status IS NOT NULL
         GROUP BY gpu_sustained_throughput_status
         ORDER BY gpu_sustained_throughput_status",
    )?;
    let mut rows = statement.query([])?;
    println!("\nGPU sustained-throughput evidence (case matrix)");
    let mut throughput_statuses = 0;
    while let Some(row) = rows.next()? {
        let status: String = row.get(0)?;
        let count: i64 = row.get(1)?;
        println!("  {status:<28} {count}");
        throughput_statuses += count;
    }
    if throughput_statuses == 0 {
        println!("  No status recorded");
    }
    Ok(())
}

pub fn rank(
    connection: &Connection,
    profile: &str,
    baseline: &str,
    below: f64,
    limit: usize,
) -> Result<()> {
    let (column, expected_backend) = match baseline.to_ascii_lowercase().as_str() {
        "pillow" => ("verified_speedup_vs_pillow", backend_for_profile(profile)),
        "pillow-simd" | "pillow_simd" => (
            "verified_speedup_vs_pillow_simd",
            backend_for_profile(profile),
        ),
        "simd" => {
            if profile != "python-gpu" {
                bail!("baseline SIMD applies to profile python-gpu");
            }
            ("verified_gpu_latency_speedup_vs_simd", Some("gpu"))
        }
        _ => bail!("baseline must be Pillow, Pillow-SIMD, or SIMD"),
    };
    let Some(expected_backend) = expected_backend else {
        bail!("unknown profile {profile:?}; expected python-cpu, python-simd, or python-gpu");
    };
    let sql = format!(
        "WITH latest AS (
             SELECT sha256 FROM artifacts
             WHERE source_kind = 'case-matrix'
             ORDER BY measured_at DESC NULLS LAST, imported_at_epoch_us DESC
             LIMIT 1
         )
         SELECT coalesce(nullif(o.raw_json->>'public_apis', ''), o.operation),
                o.workload, o.runner_id, o.mode, o.width, o.height,
                o.median_us, o.{column}, o.evidence_status, o.actual_backend,
                o.terminal_complete, o.fallback_reasons
         FROM latest l
         JOIN artifact_rows ar ON ar.artifact_sha256 = l.sha256
         JOIN observations o ON o.observation_id = ar.observation_id
         WHERE o.source_kind = 'case-matrix'
           AND o.profile = ?
           AND o.evidence_status = 'parity_backend_verified'
           AND lower(coalesce(o.status, '')) = 'completed'
           AND o.{column} IS NOT NULL AND o.{column} < ?
           AND o.terminal_complete = TRUE
           AND lower(coalesce(o.actual_backend, '')) = lower(?)
           AND (o.fallback_reasons IS NULL OR trim(o.fallback_reasons) IN ('', '{{}}', '[]', 'null'))
         ORDER BY o.{column} ASC, o.operation, o.workload
         LIMIT ?"
    );
    let mut statement = connection.prepare(&sql)?;
    let mut rows = statement.query(params![profile, below, expected_backend, limit as i64])?;
    println!("Verified {profile} cases below {below:.3}x {baseline} (latest per-case matrix)");
    println!(
        "{:<42} {:<60} {:<24} {:>11} {:>9} {:<8}",
        "OPERATION", "WORKLOAD", "RUNNER", "MEDIAN US", "SPEEDUP", "MODE"
    );
    let mut count = 0;
    while let Some(row) = rows.next()? {
        let operation: Option<String> = row.get(0)?;
        let workload: Option<String> = row.get(1)?;
        let runner: Option<String> = row.get(2)?;
        let mode: Option<String> = row.get(3)?;
        let width: Option<i64> = row.get(4)?;
        let height: Option<i64> = row.get(5)?;
        let median: Option<f64> = row.get(6)?;
        let ratio: Option<f64> = row.get(7)?;
        println!(
            "{:<42.42} {:<60.60} {:<24.24} {:>11} {:>8.3}x {:<8}",
            operation.as_deref().unwrap_or("<unknown>"),
            workload.as_deref().unwrap_or("<unknown>"),
            runner.as_deref().unwrap_or("<unknown>"),
            median.map_or_else(|| "-".into(), |v| format!("{v:.3}")),
            ratio.unwrap_or_default(),
            format!(
                "{}{}",
                mode.as_deref().unwrap_or("?"),
                width
                    .zip(height)
                    .map_or(String::new(), |(w, h)| format!(" {w}x{h}"))
            ),
        );
        count += 1;
    }
    if count == 0 {
        println!("No qualifying cases.");
    }
    Ok(())
}

pub fn history(
    connection: &Connection,
    workload: &str,
    profile: &str,
    source_kind: Option<&str>,
    limit: usize,
) -> Result<()> {
    let mut sql = String::from(
        "SELECT a.sha256, o.source_kind, o.measured_at, o.revision, o.run_id,
                o.runner_id, o.median_us, o.p95_us, o.throughput_ops_s, o.status,
                o.correctness, o.actual_backend, o.terminal_complete, o.evidence_status
         FROM artifact_rows ar
         JOIN artifacts a ON a.sha256 = ar.artifact_sha256
         JOIN observations o ON o.observation_id = ar.observation_id
         WHERE o.workload = ? AND o.profile = ?",
    );
    if source_kind.is_some() {
        sql.push_str(" AND o.source_kind = ?");
    }
    sql.push_str(
        " ORDER BY coalesce(o.measured_at, a.measured_at) ASC,
                  a.imported_at_epoch_us ASC, ar.row_ordinal ASC LIMIT ?",
    );
    let mut statement = connection.prepare(&sql)?;
    let mut rows = match source_kind {
        Some(source) => statement.query(params![workload, profile, source, limit as i64])?,
        None => statement.query(params![workload, profile, limit as i64])?,
    };
    println!("Timeline: workload={workload} profile={profile}");
    println!(
        "{:<14} {:<22} {:<25} {:<11} {:<24} {:>11} {:>9} {:<10} {:<20}",
        "ARTIFACT",
        "SOURCE",
        "MEASURED",
        "REVISION",
        "RUNNER",
        "MEDIAN US",
        "P95 US",
        "BACKEND",
        "STATUS / EVIDENCE"
    );
    let mut count = 0;
    while let Some(row) = rows.next()? {
        let artifact: String = row.get(0)?;
        let source: String = row.get(1)?;
        let measured: Option<String> = row.get(2)?;
        let revision: Option<String> = row.get(3)?;
        let run_id: Option<String> = row.get(4)?;
        let runner: Option<String> = row.get(5)?;
        let median: Option<f64> = row.get(6)?;
        let p95: Option<f64> = row.get(7)?;
        let throughput: Option<f64> = row.get(8)?;
        let status: Option<String> = row.get(9)?;
        let correctness: Option<String> = row.get(10)?;
        let backend: Option<String> = row.get(11)?;
        let terminal: Option<bool> = row.get(12)?;
        let evidence: Option<String> = row.get(13)?;
        let revision_or_run = revision.as_deref().or(run_id.as_deref()).unwrap_or("-");
        let status_text = evidence
            .as_deref()
            .or(correctness.as_deref())
            .or(status.as_deref())
            .unwrap_or("-");
        let status_text = format!(
            "{status_text}; terminal={}",
            terminal.map_or("?".into(), |v| v.to_string())
        );
        println!(
            "{:<14} {:<22.22} {:<25.25} {:<11.11} {:<24.24} {:>11} {:>9} {:<10} {}",
            short(&artifact),
            source,
            measured.as_deref().unwrap_or("-"),
            &revision_or_run[..revision_or_run.len().min(11)],
            runner.as_deref().unwrap_or("-"),
            median.map_or_else(|| "-".into(), |v| format!("{v:.3}")),
            p95.map_or_else(|| "-".into(), |v| format!("{v:.3}")),
            backend.as_deref().unwrap_or("-"),
            status_text,
        );
        if let Some(throughput) = throughput {
            println!("  throughput={throughput:.3} ops/s");
        }
        count += 1;
    }
    if count == 0 {
        println!("No matching rows. Check the workload, profile, and imported source category.");
    }
    Ok(())
}

#[derive(Debug)]
struct CompareRow {
    artifact: String,
    source_kind: String,
    status: Option<String>,
    runner_id: Option<String>,
    runner_signature: Option<String>,
    workload_signature: Option<String>,
    median_us: Option<f64>,
    evidence_status: Option<String>,
    correctness: Option<String>,
    actual_backend: Option<String>,
    terminal_complete: Option<bool>,
    fallback_reasons: Option<String>,
}

pub fn compare(
    connection: &Connection,
    workload: &str,
    profile: &str,
    source_kind: &str,
    from_prefix: &str,
    to_prefix: &str,
) -> Result<()> {
    let from_artifact = resolve_artifact(connection, from_prefix, source_kind)?;
    let to_artifact = resolve_artifact(connection, to_prefix, source_kind)?;
    if from_artifact == to_artifact {
        bail!("--from and --to resolve to the same artifact");
    }
    let from_rows = compare_rows(connection, &from_artifact, workload, profile, source_kind)?;
    let to_rows = compare_rows(connection, &to_artifact, workload, profile, source_kind)?;
    if from_rows.is_empty() || to_rows.is_empty() {
        println!(
            "No records for workload={workload} profile={profile} in both selected artifacts."
        );
        println!(
            "from={} rows={}; to={} rows={}",
            short(&from_artifact),
            from_rows.len(),
            short(&to_artifact),
            to_rows.len()
        );
        return Ok(());
    }

    let mut destination: BTreeMap<(String, String), Vec<&CompareRow>> = BTreeMap::new();
    for row in &to_rows {
        if let (Some(runner), Some(workload_signature)) =
            (&row.runner_signature, &row.workload_signature)
        {
            destination
                .entry((runner.clone(), workload_signature.clone()))
                .or_default()
                .push(row);
        }
    }

    println!(
        "Case comparison: {workload} / {profile} / {source_kind}\n  from={} ({})\n  to={} ({})",
        short(&from_artifact),
        from_rows.len(),
        short(&to_artifact),
        to_rows.len()
    );
    println!(
        "{:<24} {:>11} {:>11} {:>12} {:>10} EVIDENCE",
        "RUNNER", "FROM US", "TO US", "DELTA US", "SPEEDUP"
    );
    let mut paired = 0;
    for from in &from_rows {
        let (Some(runner_signature), Some(workload_signature)) =
            (&from.runner_signature, &from.workload_signature)
        else {
            println!(
                "  unpaired: source row lacks runner or workload signature ({})",
                short(&from.artifact)
            );
            continue;
        };
        if runner_signature.starts_with("unresolved-runner:") {
            println!(
                "  unpaired: runner identity is only a label ({})",
                from.runner_id.as_deref().unwrap_or("unknown")
            );
            continue;
        }
        let key = (runner_signature.clone(), workload_signature.clone());
        let Some(matches) = destination.get(&key) else {
            println!(
                "  unpaired: {} has no matching runner/workload-policy row in destination",
                from.runner_id.as_deref().unwrap_or("unknown")
            );
            continue;
        };
        if matches.len() != 1 {
            println!(
                "  unpaired: destination has {} rows with the same signature",
                matches.len()
            );
            continue;
        }
        let to = matches[0];
        let Some(from_latency) = from.median_us else {
            println!("  unpaired: source has no median latency");
            continue;
        };
        let Some(to_latency) = to.median_us else {
            println!("  unpaired: destination has no median latency");
            continue;
        };
        let from_valid = comparable_row(from, profile);
        let to_valid = comparable_row(to, profile);
        if !from_valid || !to_valid {
            println!("  unpaired: parity/backend/terminal/fallback evidence is insufficient");
            continue;
        }
        println!(
            "{:<24.24} {:>11.3} {:>11.3} {:>12.3} {:>9.3}x paired",
            from.runner_id.as_deref().unwrap_or("unknown"),
            from_latency,
            to_latency,
            to_latency - from_latency,
            from_latency / to_latency
        );
        paired += 1;
    }
    println!(
        "Paired cases: {paired}; source rows: {}; destination rows: {}",
        from_rows.len(),
        to_rows.len()
    );
    if paired == 0 {
        println!("No ratio was calculated because no row passed all pairing/evidence checks.");
    }
    Ok(())
}

fn comparable_row(row: &CompareRow, profile: &str) -> bool {
    let expected_backend = backend_for_profile(profile);
    let backend_matches = expected_backend.is_none_or(|expected| {
        row.actual_backend
            .as_deref()
            .is_some_and(|actual| actual.eq_ignore_ascii_case(expected))
    });
    let correctness_passed = row.correctness.as_deref().is_some_and(|text| {
        text.to_ascii_lowercase().contains("pass") && !text.to_ascii_lowercase().contains("fail")
    });
    let completed = row
        .status
        .as_deref()
        .is_some_and(|status| status.eq_ignore_ascii_case("completed"));
    let matrix_verified = row.source_kind != "case-matrix"
        || row.evidence_status.as_deref() == Some("parity_backend_verified");
    let no_fallback = row
        .fallback_reasons
        .as_deref()
        .is_none_or(|text| matches!(text.trim(), "" | "{}" | "[]" | "null"));
    matrix_verified
        && completed
        && backend_matches
        && correctness_passed
        && row.terminal_complete == Some(true)
        && no_fallback
}

fn backend_for_profile(profile: &str) -> Option<&'static str> {
    match profile.to_ascii_lowercase().as_str() {
        "python-cpu" => Some("cpu"),
        "python-simd" => Some("simd"),
        "python-gpu" => Some("gpu"),
        _ => None,
    }
}

fn resolve_artifact(connection: &Connection, prefix: &str, source_kind: &str) -> Result<String> {
    let mut statement = connection
        .prepare("SELECT sha256 FROM artifacts WHERE starts_with(sha256, ?) AND source_kind = ?")?;
    let hashes = statement
        .query_map(params![prefix, source_kind], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    match hashes.as_slice() {
        [hash] => Ok(hash.clone()),
        [] => bail!("no {source_kind} artifact matches SHA prefix {prefix:?}"),
        _ => bail!("SHA prefix {prefix:?} matches multiple {source_kind} artifacts"),
    }
}

fn compare_rows(
    connection: &Connection,
    artifact: &str,
    workload: &str,
    profile: &str,
    source_kind: &str,
) -> Result<Vec<CompareRow>> {
    let mut statement = connection.prepare(
        "SELECT a.sha256, o.source_kind, o.status, o.runner_id, o.runner_signature,
                o.workload_signature, o.median_us, o.evidence_status, o.correctness,
                o.actual_backend, o.terminal_complete, o.fallback_reasons
         FROM artifact_rows ar
         JOIN artifacts a ON a.sha256 = ar.artifact_sha256
         JOIN observations o ON o.observation_id = ar.observation_id
         WHERE ar.artifact_sha256 = ? AND o.workload = ? AND o.profile = ?
           AND o.source_kind = ?",
    )?;
    let result = statement
        .query_map(params![artifact, workload, profile, source_kind], |row| {
            Ok(CompareRow {
                artifact: row.get(0)?,
                source_kind: row.get(1)?,
                status: row.get(2)?,
                runner_id: row.get(3)?,
                runner_signature: row.get(4)?,
                workload_signature: row.get(5)?,
                median_us: row.get(6)?,
                evidence_status: row.get(7)?,
                correctness: row.get(8)?,
                actual_backend: row.get(9)?,
                terminal_complete: row.get(10)?,
                fallback_reasons: row.get(11)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(result)
}

pub fn print_seed_summary(connection: &Connection, imported: usize, skipped: usize) -> Result<()> {
    let artifacts: i64 = connection
        .query_row("SELECT count(*) FROM artifacts", [], |row| row.get(0))
        .context("count imported artifacts")?;
    let rows: i64 = connection
        .query_row("SELECT count(*) FROM artifact_rows", [], |row| row.get(0))
        .context("count imported artifact rows")?;
    println!(
        "Seed complete: added {imported} artifacts, skipped {skipped} unchanged; database has {artifacts} artifacts and {rows} source rows."
    );
    Ok(())
}
