pub mod rules;
pub mod size;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use globset::{Glob, GlobSet, GlobSetBuilder};
use jwalk::WalkDir;
use rayon::prelude::*;

use crate::error::CleanerError;
use crate::scanner::rules::{classify_directory, is_protected_system_path, is_vcs_directory};
use crate::scanner::size::compute_directory_stats;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    NodeModules,
    DotNetBin,
    DotNetObj,
    NuGetPackages,
}

impl ArtifactKind {
    pub fn ecosystem_label(self) -> &'static str {
        match self {
            ArtifactKind::NodeModules => "Node.js",
            ArtifactKind::DotNetBin | ArtifactKind::DotNetObj | ArtifactKind::NuGetPackages => {
                ".NET / NuGet"
            }
        }
    }

    pub fn short_kind(self) -> &'static str {
        match self {
            ArtifactKind::NodeModules => "node_modules",
            ArtifactKind::DotNetBin => "bin",
            ArtifactKind::DotNetObj => "obj",
            ArtifactKind::NuGetPackages => "packages",
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CleanTarget {
    pub path: PathBuf,
    pub kind: ArtifactKind,
    pub size_bytes: u64,
    pub file_count: u64,
    #[serde(serialize_with = "serialize_optional_system_time")]
    pub last_modified: Option<SystemTime>,
}

fn serialize_optional_system_time<S>(
    time: &Option<SystemTime>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match time {
        Some(t) => {
            let secs = t
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            serializer.serialize_some(&secs)
        }
        None => serializer.serialize_none(),
    }
}

#[derive(Debug, Clone)]
pub struct ScanConfig {
    pub root_path: PathBuf,
    pub scan_node: bool,
    pub scan_nuget: bool,
    pub include_obj: bool,
    pub strict_node: bool,
    pub older_than: Option<Duration>,
    pub exclude_patterns: Vec<String>,
}

/// Scans `config.root_path` in parallel, prunes matched target subtrees immediately,
/// and computes directory sizes in parallel via `rayon`.
pub fn scan_directory(config: &ScanConfig) -> Result<Vec<CleanTarget>, CleanerError> {
    if !config.root_path.exists() {
        return Err(CleanerError::TargetNotFound(config.root_path.clone()));
    }
    if !config.root_path.is_dir() {
        return Err(CleanerError::TargetNotDirectory(config.root_path.clone()));
    }

    let canonical_root = config
        .root_path
        .canonicalize()
        .unwrap_or_else(|_| config.root_path.clone());

    if is_protected_system_path(&canonical_root) {
        return Err(CleanerError::ProtectedSystemPath(canonical_root));
    }

    let exclude_set = build_glob_set(&config.exclude_patterns)?;
    let discovered: Arc<Mutex<Vec<(PathBuf, ArtifactKind)>>> = Arc::new(Mutex::new(Vec::new()));

    let scan_node = config.scan_node;
    let scan_nuget = config.scan_nuget;
    let include_obj = config.include_obj;
    let strict_node = config.strict_node;
    let discovered_clone = Arc::clone(&discovered);

    let walker = WalkDir::new(&config.root_path)
        .skip_hidden(false)
        .follow_links(false)
        .process_read_dir(move |_depth, _path, _read_dir_state, children| {
            children.retain_mut(|dir_entry_result| {
                let Ok(dir_entry) = dir_entry_result else {
                    return true;
                };

                if !dir_entry.file_type().is_dir() {
                    return false; // We only need to inspect directories during the discovery walk
                }

                let entry_path = dir_entry.path();
                let Some(dir_name) = entry_path.file_name().and_then(|n| n.to_str()) else {
                    return true;
                };

                // Skip hidden VCS directories (.git, .hg, .svn)
                if is_vcs_directory(dir_name) {
                    dir_entry.read_children_path = None;
                    return false;
                }

                // Skip user-excluded glob patterns
                if exclude_set.is_match(&entry_path) || exclude_set.is_match(dir_name) {
                    dir_entry.read_children_path = None;
                    return false;
                }

                // Check if this directory is a target (`node_modules`, `bin`, `obj`, `packages`)
                if let Some(kind) = classify_directory(
                    &entry_path,
                    dir_name,
                    scan_node,
                    scan_nuget,
                    include_obj,
                    strict_node,
                ) {
                    // Prune descent so we never recurse into nested node_modules or bin folders!
                    dir_entry.read_children_path = None;
                    if let Ok(mut guard) = discovered_clone.lock() {
                        guard.push((entry_path, kind));
                    }
                    return false;
                }

                true
            });
        });

    // Drive the parallel directory walker to completion
    for _ in walker {}

    let candidates = discovered
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default();

    let now = SystemTime::now();

    // Compute sizes and filter by `--older-than` in parallel
    let mut targets: Vec<CleanTarget> = candidates
        .into_par_iter()
        .filter_map(|(path, kind)| {
            let stats = compute_directory_stats(&path);

            if let Some(min_age) = config.older_than {
                let is_old_enough = stats
                    .last_modified
                    .and_then(|mtime| now.duration_since(mtime).ok())
                    .map(|age| age >= min_age)
                    .unwrap_or(false);

                if !is_old_enough {
                    return None;
                }
            }

            Some(CleanTarget {
                path,
                kind,
                size_bytes: stats.size_bytes,
                file_count: stats.file_count,
                last_modified: stats.last_modified,
            })
        })
        .collect();

    // Sort largest size first (with path tie-breaker for deterministic output)
    targets.sort_by(|a, b| {
        b.size_bytes
            .cmp(&a.size_bytes)
            .then_with(|| a.path.cmp(&b.path))
    });

    Ok(targets)
}

fn build_glob_set(patterns: &[String]) -> Result<GlobSet, CleanerError> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = Glob::new(pattern)
            .map_err(|e| CleanerError::InvalidGlob(pattern.clone(), e.to_string()))?;
        builder.add(glob);
    }
    builder
        .build()
        .map_err(|e| CleanerError::InvalidGlob(patterns.join(", "), e.to_string()))
}
