use std::collections::HashMap;
use std::{fs, path::Path};

use anyhow::{Context, Result};
use duckdb::{Connection, params};
use serde_json::Value;

use crate::{
    import::ArtifactImport,
    model::{Observation, signature},
};

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS perfdb_meta (
    key VARCHAR PRIMARY KEY,
    value VARCHAR NOT NULL
);
INSERT INTO perfdb_meta VALUES ('schema_version', '3')
ON CONFLICT (key) DO NOTHING;

CREATE TABLE IF NOT EXISTS artifacts (
    sha256 VARCHAR PRIMARY KEY,
    source_path VARCHAR NOT NULL,
    format VARCHAR NOT NULL,
    source_kind VARCHAR NOT NULL,
    schema_name VARCHAR,
    repository VARCHAR,
    revision VARCHAR,
    measured_at VARCHAR,
    run_id VARCHAR,
    imported_at_epoch_us BIGINT NOT NULL,
    row_count BIGINT NOT NULL,
    metadata_json JSON NOT NULL
);

CREATE TABLE IF NOT EXISTS observations (
    observation_id VARCHAR PRIMARY KEY,
    source_kind VARCHAR NOT NULL,
    record_kind VARCHAR NOT NULL,
    workload VARCHAR,
    operation VARCHAR,
    scope VARCHAR,
    operation_class VARCHAR,
    subject VARCHAR,
    profile VARCHAR,
    baseline VARCHAR,
    comparison_group VARCHAR,
    run_id VARCHAR,
    revision VARCHAR,
    measured_at VARCHAR,
    runner_id VARCHAR,
    runner_signature VARCHAR,
    workload_signature VARCHAR,
    mode VARCHAR,
    width BIGINT,
    height BIGINT,
    chain_length BIGINT,
    boundary VARCHAR,
    sample_unit VARCHAR,
    sample_count BIGINT,
    median_us DOUBLE,
    p95_us DOUBLE,
    throughput_ops_s DOUBLE,
    verified_speedup_vs_pillow DOUBLE,
    verified_speedup_vs_pillow_simd DOUBLE,
    verified_gpu_latency_speedup_vs_simd DOUBLE,
    gpu_sustained_throughput_status VARCHAR,
    status VARCHAR,
    correctness VARCHAR,
    evidence_status VARCHAR,
    requested_backend VARCHAR,
    actual_backend VARCHAR,
    terminal_complete BOOLEAN,
    fallback_reasons VARCHAR,
    policy_json JSON,
    context_json JSON,
    resource_json JSON,
    raw_json JSON NOT NULL
);

CREATE TABLE IF NOT EXISTS artifact_rows (
    artifact_sha256 VARCHAR NOT NULL,
    row_ordinal BIGINT NOT NULL,
    observation_id VARCHAR NOT NULL,
    record_kind VARCHAR NOT NULL,
    PRIMARY KEY (artifact_sha256, row_ordinal)
);
DROP INDEX IF EXISTS observations_workload_profile_idx;
DROP INDEX IF EXISTS observations_source_runner_idx;
"#;

pub struct Store {
    pub connection: Connection,
    pub path: String,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)
                .with_context(|| format!("create database directory {}", parent.display()))?;
        }
        let connection = Connection::open(path)
            .with_context(|| format!("open DuckDB database {}", path.display()))?;
        connection.execute_batch(SCHEMA)?;
        let has_duplicate_raw: bool = connection.query_row(
            "SELECT count(*) > 0 FROM pragma_table_info('artifact_rows') WHERE name = 'raw_json'",
            [],
            |row| row.get(0),
        )?;
        if has_duplicate_raw {
            connection.execute_batch("ALTER TABLE artifact_rows DROP COLUMN raw_json")?;
        }
        let mut added_observation_columns = false;
        for (name, column_type) in [
            ("scope", "VARCHAR"),
            ("operation_class", "VARCHAR"),
            ("gpu_sustained_throughput_status", "VARCHAR"),
        ] {
            let present: bool = connection.query_row(
                "SELECT count(*) > 0 FROM pragma_table_info('observations') WHERE name = ?",
                params![name],
                |row| row.get(0),
            )?;
            if !present {
                connection.execute_batch(&format!(
                    "ALTER TABLE observations ADD COLUMN {name} {column_type}"
                ))?;
                added_observation_columns = true;
            }
        }
        if added_observation_columns {
            connection.execute_batch(
                "UPDATE observations SET
                    scope = COALESCE(
                        scope,
                        NULLIF(raw_json->>'scope', ''),
                        CASE
                            WHEN workload LIKE 'pipeline-chain.%' THEN 'composed_pipeline'
                            WHEN workload LIKE 'pipeline-op.%' OR workload LIKE 'pil-image-%'
                                THEN 'individual_operation'
                            ELSE NULL
                        END
                    ),
                    operation_class = COALESCE(
                        operation_class,
                        NULLIF(raw_json->>'operation_class', ''),
                        NULLIF(context_json->>'operation_class', ''),
                        json_extract_string(raw_json, '$.record.context.operation_class')
                    ),
                    gpu_sustained_throughput_status = COALESCE(
                        gpu_sustained_throughput_status,
                        NULLIF(raw_json->>'gpu_sustained_throughput_status', ''),
                        NULLIF(resource_json->>'gpu_sustained_throughput', '')
                    )",
            )?;
        }
        connection.execute(
            "UPDATE perfdb_meta SET value = '3' WHERE key = 'schema_version'",
            [],
        )?;
        Ok(Self {
            connection,
            path: path.display().to_string(),
        })
    }

    pub fn import(&mut self, artifact: ArtifactImport) -> Result<bool> {
        let exists: i64 = self.connection.query_row(
            "SELECT count(*) FROM artifacts WHERE sha256 = ?",
            params![artifact.sha256],
            |row| row.get(0),
        )?;
        if exists != 0 {
            let refreshed_runners = if artifact.source_kind == "case-matrix" {
                self.refresh_matrix_runners(&artifact)?
            } else {
                0
            };
            println!(
                "unchanged {} {} rows={} runner-identities-refreshed={} sha256={}",
                artifact.source_kind,
                artifact.source_path,
                artifact.rows.len(),
                refreshed_runners,
                short(&artifact.sha256)
            );
            return Ok(false);
        }

        self.connection.execute_batch("BEGIN TRANSACTION")?;
        let result = self.import_transaction(&artifact);
        match result {
            Ok(()) => {
                self.connection.execute_batch("COMMIT")?;
                println!(
                    "imported {} {} rows={} sha256={}",
                    artifact.source_kind,
                    artifact.source_path,
                    artifact.rows.len(),
                    short(&artifact.sha256)
                );
                Ok(true)
            }
            Err(error) => {
                let _ = self.connection.execute_batch("ROLLBACK");
                Err(error)
            }
        }
    }

    fn refresh_matrix_runners(&self, artifact: &ArtifactImport) -> Result<usize> {
        self.connection.execute_batch("BEGIN TRANSACTION")?;
        let result = (|| {
            let mut update_statement = self.connection.prepare(
                "UPDATE observations SET runner_signature = ? WHERE observation_id = ?
                 AND (runner_signature IS NULL OR starts_with(runner_signature, 'unresolved-runner:'))",
            )?;
            let mut metadata_cache = HashMap::<String, Value>::new();
            let mut refreshed = 0;
            for source_observation in &artifact.rows {
                let mut observation = source_observation.clone();
                self.resolve_matrix_runner(&mut observation, &mut metadata_cache)?;
                let Some(ref runner_signature) = observation.runner_signature else {
                    continue;
                };
                if runner_signature.starts_with("unresolved-runner:") {
                    continue;
                }
                let observation_id = observation.observation_id()?;
                refreshed += update_statement.execute(params![runner_signature, observation_id])?;
            }
            Ok::<usize, anyhow::Error>(refreshed)
        })();
        match result {
            Ok(refreshed) => {
                self.connection.execute_batch("COMMIT")?;
                Ok(refreshed)
            }
            Err(error) => {
                let _ = self.connection.execute_batch("ROLLBACK");
                Err(error)
            }
        }
    }

    fn import_transaction(&self, artifact: &ArtifactImport) -> Result<()> {
        let now_us = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_micros() as i64;
        let metadata = serde_json::to_string(&artifact.metadata)?;
        self.connection.execute(
            "INSERT INTO artifacts (
                sha256, source_path, format, source_kind, schema_name, repository,
                revision, measured_at, run_id, imported_at_epoch_us, row_count,
                metadata_json
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, CAST(? AS JSON))",
            params![
                artifact.sha256,
                artifact.source_path,
                artifact.format,
                artifact.source_kind,
                artifact.schema_name,
                artifact.repository,
                artifact.revision,
                artifact.measured_at,
                artifact.run_id,
                now_us,
                artifact.rows.len() as i64,
                metadata,
            ],
        )?;

        let mut observation_statement = self.connection.prepare(
            "INSERT INTO observations (
                observation_id, source_kind, record_kind, workload, operation, scope,
                operation_class, subject, profile, baseline, comparison_group, run_id,
                revision, measured_at,
                runner_id, runner_signature, workload_signature, mode, width, height,
                chain_length, boundary, sample_unit, sample_count, median_us, p95_us,
                throughput_ops_s, verified_speedup_vs_pillow,
                verified_speedup_vs_pillow_simd,
                verified_gpu_latency_speedup_vs_simd, gpu_sustained_throughput_status,
                status, correctness,
                evidence_status, requested_backend, actual_backend, terminal_complete,
                fallback_reasons, policy_json, context_json, resource_json, raw_json
            ) VALUES (
                ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
                ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, CAST(? AS JSON), CAST(? AS JSON),
                CAST(? AS JSON), CAST(? AS JSON)
            ) ON CONFLICT (observation_id) DO NOTHING",
        )?;
        let mut link_statement = self.connection.prepare(
            "INSERT INTO artifact_rows (
                artifact_sha256, row_ordinal, observation_id, record_kind
            ) VALUES (?, ?, ?, ?)",
        )?;
        let mut machine_metadata = HashMap::<String, Value>::new();

        for (ordinal, observation) in artifact.rows.iter().enumerate() {
            let mut observation = observation.clone();
            self.resolve_matrix_runner(&mut observation, &mut machine_metadata)?;
            let id = observation.observation_id()?;
            let raw_json = observation.row_json()?;
            let policy_json = json_string(&observation.policy)?;
            let context_json = json_string(&observation.context)?;
            let resource_json = json_string(&observation.resource)?;
            observation_statement.execute(params![
                id,
                observation.source_kind,
                observation.record_kind,
                observation.workload,
                observation.operation,
                observation.scope,
                observation.operation_class,
                observation.subject,
                observation.profile,
                observation.baseline,
                observation.comparison_group,
                observation.run_id,
                observation.revision,
                observation.measured_at,
                observation.runner_id,
                observation.runner_signature,
                observation.workload_signature,
                observation.mode,
                observation.width,
                observation.height,
                observation.chain_length,
                observation.boundary,
                observation.sample_unit,
                observation.sample_count,
                observation.median_us,
                observation.p95_us,
                observation.throughput_ops_s,
                observation.verified_speedup_vs_pillow,
                observation.verified_speedup_vs_pillow_simd,
                observation.verified_gpu_latency_speedup_vs_simd,
                observation.gpu_sustained_throughput_status,
                observation.status,
                observation.correctness,
                observation.evidence_status,
                observation.requested_backend,
                observation.actual_backend,
                observation.terminal_complete,
                observation.fallback_reasons,
                policy_json,
                context_json,
                resource_json,
                raw_json,
            ])?;
            link_statement.execute(params![
                artifact.sha256,
                ordinal as i64,
                id,
                observation.record_kind,
            ])?;
        }
        Ok(())
    }

    fn resolve_matrix_runner(
        &self,
        observation: &mut Observation,
        metadata_cache: &mut HashMap<String, Value>,
    ) -> Result<()> {
        if observation.source_kind != "case-matrix" {
            return Ok(());
        }
        let source_sha = observation
            .raw
            .get("source_json_sha256")
            .and_then(Value::as_str);
        let Some(source_sha) = source_sha else {
            return Ok(());
        };
        let Some(machine_id) = observation.runner_id.as_deref() else {
            return Ok(());
        };
        if !metadata_cache.contains_key(source_sha) {
            let metadata: Option<String> = self
                .connection
                .query_row(
                    "SELECT CAST(metadata_json AS VARCHAR) FROM artifacts WHERE sha256 = ?",
                    params![source_sha],
                    |row| row.get(0),
                )
                .ok();
            if let Some(metadata) = metadata {
                metadata_cache.insert(source_sha.to_owned(), serde_json::from_str(&metadata)?);
            } else {
                return Ok(());
            }
        }
        let Some(metadata) = metadata_cache.get(source_sha) else {
            return Ok(());
        };
        let machine = metadata
            .get("machines")
            .and_then(Value::as_array)
            .and_then(|machines| {
                machines
                    .iter()
                    .find(|machine| machine.get("id").and_then(Value::as_str) == Some(machine_id))
            })
            .or_else(|| {
                metadata
                    .get("machine")
                    .filter(|machine| machine.get("id").and_then(Value::as_str) == Some(machine_id))
            });
        if let Some(machine) = machine {
            observation.runner_signature = Some(signature(machine));
        }
        Ok(())
    }
}

pub fn short(value: &str) -> &str {
    &value[..value.len().min(12)]
}

fn json_string(value: &Option<Value>) -> Result<Option<String>> {
    value
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(Into::into)
}
