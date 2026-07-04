#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
#![allow(unused_crate_dependencies)]
#![allow(missing_docs)]

use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct InterfaceMap {
    paths: Vec<InterfacePath>,
}

#[derive(Debug, Deserialize)]
struct InterfacePath {
    path: String,
    #[allow(dead_code)]
    owner: String,
    #[serde(default)]
    parity: Option<Parity>,
    symbols: BTreeMap<String, SymbolMapping>,
}

#[derive(Debug, Deserialize)]
struct SymbolMapping {
    #[allow(dead_code)]
    rust: Option<String>,
    status: Status,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct Parity {
    family: String,
    passing: u32,
    total: u32,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Status {
    Complete,
    Partial,
    Planned,
    OutOfScope,
}

#[derive(Default)]
struct PathStats {
    complete: u32,
    partial: u32,
    planned: u32,
    out_of_scope: u32,
    missing_from_headers: u32,
    parity: Option<Parity>,
}

#[test]
fn freetype_interface_coverage_report() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let header_root = manifest_dir
        .join("freetype")
        .join("include")
        .join("freetype");
    let exported_symbols = discover_ft_exports(&header_root);

    let map_path = manifest_dir
        .join("tests")
        .join("fixtures")
        .join("interface_map.json");
    let interface_map: InterfaceMap =
        serde_json::from_str(&fs::read_to_string(&map_path).expect("read interface_map.json"))
            .expect("parse interface_map.json");

    let mut mapped_symbols = BTreeSet::new();
    let mut path_stats = BTreeMap::<String, PathStats>::new();

    for path in &interface_map.paths {
        let stats = path_stats.entry(path.path.clone()).or_default();
        stats.parity = path.parity.clone();

        for (symbol, mapping) in &path.symbols {
            mapped_symbols.insert(symbol.clone());
            if !exported_symbols.contains(symbol) {
                stats.missing_from_headers += 1;
            }
            if mapping.status == Status::OutOfScope {
                assert!(
                    mapping
                        .reason
                        .as_ref()
                        .is_some_and(|reason| !reason.trim().is_empty()),
                    "{symbol} is out_of_scope but has no reason"
                );
            }
            match mapping.status {
                Status::Complete => stats.complete += 1,
                Status::Partial => stats.partial += 1,
                Status::Planned => stats.planned += 1,
                Status::OutOfScope => stats.out_of_scope += 1,
            }
        }
    }

    let in_scope_exports = exported_symbols
        .iter()
        .filter(|symbol| {
            interface_map.paths.iter().all(|path| {
                path.symbols
                    .get(*symbol)
                    .is_none_or(|mapping| mapping.status != Status::OutOfScope)
            })
        })
        .count() as u32;
    let complete = count_status(&interface_map, Status::Complete);
    let partial = count_status(&interface_map, Status::Partial);
    let planned = count_status(&interface_map, Status::Planned);
    let out_of_scope = count_status(&interface_map, Status::OutOfScope);
    let mapped_in_headers = mapped_symbols
        .iter()
        .filter(|symbol| exported_symbols.contains(*symbol))
        .count() as u32;
    let unmapped = exported_symbols.len() as u32 - mapped_in_headers;
    let implemented = complete + partial;
    let api_coverage = percent(implemented, in_scope_exports);
    let complete_coverage = percent(complete, in_scope_exports);

    eprintln!("╔══════════════════════════════════════════════════════════════╗");
    eprintln!("║  FreeType Interface Coverage");
    eprintln!("╠══════════════════════════════════════════════════════════════╣");
    eprintln!(
        "║  exports={} mapped={} unmapped={} out_of_scope={}",
        exported_symbols.len(),
        mapped_in_headers,
        unmapped,
        out_of_scope
    );
    eprintln!(
        "║  implemented={} complete={} partial={} planned={}",
        implemented, complete, partial, planned
    );
    eprintln!("║  api_coverage={api_coverage:.1}% complete_coverage={complete_coverage:.1}%");
    eprintln!("╠══════════════════════════════════════════════════════════════╣");
    eprintln!("║  Path coverage");

    for (path, stats) in &path_stats {
        let total = stats.complete + stats.partial + stats.planned + stats.out_of_scope;
        let implemented = stats.complete + stats.partial;
        let parity = stats
            .parity
            .as_ref()
            .map(|p| {
                format!(
                    "{} {}/{} ({:.1}%)",
                    p.family,
                    p.passing,
                    p.total,
                    percent(p.passing, p.total)
                )
            })
            .unwrap_or_else(|| "no parity fixture".to_string());
        eprintln!(
            "║  {path:<24} api {implemented:>3}/{total:<3} {:>5.1}%  complete {:>3} partial {:>3} planned {:>3}  parity {parity}",
            percent(implemented, total),
            stats.complete,
            stats.partial,
            stats.planned
        );
    }

    eprintln!("╚══════════════════════════════════════════════════════════════╝");

    assert!(
        exported_symbols.len() >= 200,
        "FreeType export parser found too few symbols: {}",
        exported_symbols.len()
    );
    assert_eq!(unmapped, 0, "all FreeType exports must be mapped");
}

fn count_status(interface_map: &InterfaceMap, status: Status) -> u32 {
    interface_map
        .paths
        .iter()
        .flat_map(|path| path.symbols.values())
        .filter(|mapping| mapping.status == status)
        .count() as u32
}

fn percent(numerator: u32, denominator: u32) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        100.0 * numerator as f64 / denominator as f64
    }
}

fn discover_ft_exports(root: &Path) -> BTreeSet<String> {
    let mut headers = Vec::new();
    collect_headers(root, &mut headers);

    let mut symbols = BTreeSet::new();
    for header in headers {
        let text = fs::read_to_string(&header).unwrap_or_else(|e| {
            panic!("read header {}: {e}", header.display());
        });
        let lines: Vec<&str> = text.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if line.contains("FT_EXPORT(") {
                if let Some(symbol) = parse_symbol_after_export(&lines[idx + 1..]) {
                    symbols.insert(symbol);
                }
            }
        }
    }

    symbols
}

fn collect_headers(dir: &Path, headers: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("read dir {}: {e}", dir.display())) {
        let entry = entry.expect("read dir entry");
        let path = entry.path();
        if path.is_dir() {
            collect_headers(&path, headers);
        } else if path.extension().is_some_and(|extension| extension == "h") {
            headers.push(path);
        }
    }
}

fn parse_symbol_after_export(lines: &[&str]) -> Option<String> {
    let mut signature = String::new();
    for line in lines.iter().take(8) {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('*') {
            continue;
        }
        signature.push_str(trimmed);
        signature.push(' ');
        if trimmed.contains('(') {
            break;
        }
    }

    let open = signature.find('(')?;
    let before_open = signature[..open].trim_end();
    before_open
        .split(|c: char| c.is_whitespace() || c == '*')
        .filter(|part| !part.is_empty())
        .next_back()
        .map(str::to_string)
}
