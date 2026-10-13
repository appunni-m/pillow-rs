mod import;
mod model;
mod report;
mod store;

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use duckdb::params;

use crate::{import::read_artifact, store::Store};

#[derive(Debug, Parser)]
#[command(
    name = "pillow-perfdb",
    version,
    about = "Import and compare pillow-rs benchmark evidence"
)]
struct Cli {
    /// DuckDB file. Defaults to tools/perfdb/performance-results.duckdb.
    #[arg(long, global = true)]
    db: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create the database and schema.
    Init,

    /// Import JSON/CSV files or recursively import a directory.
    Import {
        #[arg(required = true, num_args = 1..)]
        paths: Vec<PathBuf>,
    },

    /// Import all JSON/CSV evidence artifacts from a directory.
    Seed {
        #[arg(long, default_value = "docs/evidence")]
        evidence_dir: PathBuf,
    },

    /// Show artifact counts and evidence-status totals.
    Status,

    /// List imported artifact hashes for history/compare selection.
    Artifacts {
        #[arg(long)]
        source_kind: Option<String>,
        #[arg(long, default_value_t = 100)]
        limit: usize,
    },

    /// Rank verified cases below a latency-speedup threshold.
    Rank {
        #[arg(long, default_value = "python-cpu")]
        profile: String,
        #[arg(long, default_value = "Pillow")]
        baseline: String,
        #[arg(long, default_value_t = 1.0)]
        below: f64,
        #[arg(long, default_value_t = 200)]
        limit: usize,
    },

    /// Show one workload/profile over all imported evidence snapshots.
    History {
        #[arg(long)]
        workload: String,
        #[arg(long)]
        profile: String,
        #[arg(long)]
        source_kind: Option<String>,
        #[arg(long, default_value_t = 500)]
        limit: usize,
    },

    /// Compare one case between two artifact SHA prefixes.
    Compare {
        #[arg(long)]
        workload: String,
        #[arg(long)]
        profile: String,
        #[arg(long)]
        source_kind: String,
        #[arg(long = "from")]
        from_artifact: String,
        #[arg(long = "to")]
        to_artifact: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let db_path = cli.db.unwrap_or_else(default_db_path);
    let mut store = Store::open(&db_path)?;

    match cli.command {
        Command::Init => {
            println!("Initialized {}", store.path);
        }
        Command::Import { paths } => {
            let files = collect_paths(paths)?;
            let ordered = order_json_before_csv(files);
            let mut imported = 0;
            let mut skipped = 0;
            for path in ordered {
                if store.import(read_artifact(&path)?)? {
                    imported += 1;
                } else {
                    skipped += 1;
                }
            }
            report::print_seed_summary(&store.connection, imported, skipped)?;
        }
        Command::Seed { evidence_dir } => {
            if !evidence_dir.is_dir() {
                bail!(
                    "evidence directory does not exist: {}",
                    evidence_dir.display()
                );
            }
            let files = collect_direct_children(&evidence_dir)?;
            let ordered = order_json_before_csv(files);
            let mut imported = 0;
            let mut skipped = 0;
            for path in ordered {
                if store.import(read_artifact(&path)?)? {
                    imported += 1;
                } else {
                    skipped += 1;
                }
            }
            report::print_seed_summary(&store.connection, imported, skipped)?;
        }
        Command::Status => {
            println!("Database: {}", store.path);
            report::status(&store.connection)?;
        }
        Command::Artifacts { source_kind, limit } => {
            list_artifacts(&store.connection, source_kind.as_deref(), limit)?;
        }
        Command::Rank {
            profile,
            baseline,
            below,
            limit,
        } => report::rank(&store.connection, &profile, &baseline, below, limit)?,
        Command::History {
            workload,
            profile,
            source_kind,
            limit,
        } => report::history(
            &store.connection,
            &workload,
            &profile,
            source_kind.as_deref(),
            limit,
        )?,
        Command::Compare {
            workload,
            profile,
            source_kind,
            from_artifact,
            to_artifact,
        } => report::compare(
            &store.connection,
            &workload,
            &profile,
            &source_kind,
            &from_artifact,
            &to_artifact,
        )?,
    }
    Ok(())
}

fn default_db_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("performance-results.duckdb")
}

fn collect_paths(paths: Vec<PathBuf>) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for path in paths {
        if path.is_dir() {
            collect_recursive(&path, &mut files)?;
        } else if path.is_file() {
            files.push(path);
        } else {
            bail!("input path does not exist: {}", path.display());
        }
    }
    files.sort();
    Ok(files)
}

fn collect_recursive(path: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    let mut children = fs::read_dir(path)
        .with_context(|| format!("read directory {}", path.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    children.sort();
    for child in children {
        if child.is_dir() {
            let name = child
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            if matches!(name, ".git" | "target" | ".venv" | "node_modules") {
                continue;
            }
            collect_recursive(&child, files)?;
        } else if is_supported_file(&child) {
            files.push(child);
        }
    }
    Ok(())
}

fn collect_direct_children(path: &Path) -> Result<Vec<PathBuf>> {
    let mut files = fs::read_dir(path)
        .with_context(|| format!("read evidence directory {}", path.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|child| child.is_file() && is_supported_file(child))
        .collect::<Vec<_>>();
    files.sort();
    Ok(files)
}

fn is_supported_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "json" | "csv"
    )
}

fn order_json_before_csv(mut files: Vec<PathBuf>) -> Vec<PathBuf> {
    files.sort_by(|left, right| {
        let rank = |path: &Path| match path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "json" => 0,
            "csv" => 1,
            _ => 2,
        };
        rank(left).cmp(&rank(right)).then_with(|| left.cmp(right))
    });
    files
}

fn list_artifacts(
    connection: &duckdb::Connection,
    source_kind: Option<&str>,
    limit: usize,
) -> Result<()> {
    let base = "SELECT sha256, source_kind, source_path, revision, measured_at, row_count
                FROM artifacts";
    let sql = if source_kind.is_some() {
        format!(
            "{base} WHERE source_kind = ? ORDER BY measured_at DESC NULLS LAST, imported_at_epoch_us DESC LIMIT ?"
        )
    } else {
        format!("{base} ORDER BY measured_at DESC NULLS LAST, imported_at_epoch_us DESC LIMIT ?")
    };
    let mut statement = connection.prepare(&sql)?;
    let mut rows = match source_kind {
        Some(source) => statement.query(params![source, limit as i64])?,
        None => statement.query(params![limit as i64])?,
    };
    println!(
        "{:<14} {:<24} {:>8} {:<24} {:<12} PATH",
        "SHA256", "SOURCE", "ROWS", "MEASURED", "REVISION"
    );
    while let Some(row) = rows.next()? {
        let sha: String = row.get(0)?;
        let source: String = row.get(1)?;
        let path: String = row.get(2)?;
        let revision: Option<String> = row.get(3)?;
        let measured: Option<String> = row.get(4)?;
        let count: i64 = row.get(5)?;
        let rev = revision.as_deref().unwrap_or("-");
        println!(
            "{:<14} {:<24.24} {:>8} {:<24.24} {:<12.12} {}",
            crate::store::short(&sha),
            source,
            count,
            measured.as_deref().unwrap_or("-"),
            rev,
            path
        );
    }
    Ok(())
}
