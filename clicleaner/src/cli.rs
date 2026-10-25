use std::path::PathBuf;
use std::time::Duration;

use clap::{ArgAction, Parser, ValueEnum};

use crate::error::CleanerError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetEcosystem {
    /// Scan both Node.js (`node_modules`) and .NET/NuGet (`bin`, `obj`, `packages`)
    All,
    /// Scan only Node.js `node_modules` directories
    Node,
    /// Scan only .NET and NuGet (`bin`, `obj`, `packages`) directories
    Nuget,
}

impl TargetEcosystem {
    pub fn includes_node(self) -> bool {
        matches!(self, TargetEcosystem::All | TargetEcosystem::Node)
    }

    pub fn includes_nuget(self) -> bool {
        matches!(self, TargetEcosystem::All | TargetEcosystem::Nuget)
    }
}

/// Fast, context-aware CLI tool to scan and clean `node_modules` and .NET/NuGet (`bin`, `obj`, `packages`) directories.
#[derive(Debug, Clone, Parser)]
#[command(
    name = "clicleaner",
    version,
    about = "Scan and clean node_modules and .NET/NuGet build & package directories to free disk space",
    long_about = None
)]
pub struct CliArgs {
    /// Root directory path to scan for cleanable artifacts
    #[arg(value_name = "TARGET_PATH", default_value = ".")]
    pub target_path: PathBuf,

    /// Target ecosystems to scan (`all`, `node`, `nuget`), comma-separated
    #[arg(
        short = 't',
        long = "target",
        value_delimiter = ',',
        default_value = "all"
    )]
    pub targets: Vec<TargetEcosystem>,

    /// Include .NET `obj/` directories alongside `bin/` and NuGet `packages/`
    #[arg(
        long = "include-obj",
        default_value_t = true,
        action = ArgAction::Set,
        num_args = 0..=1,
        default_missing_value = "true"
    )]
    pub include_obj: bool,

    /// Require a sibling `package.json` or lockfile before flagging `node_modules`
    #[arg(long = "strict", default_value_t = false)]
    pub strict: bool,

    /// Scan and display reclaimable space without deleting anything
    #[arg(short = 'd', long = "dry-run", default_value_t = false)]
    pub dry_run: bool,

    /// Present an interactive multi-select list of discovered folders before deleting
    #[arg(short = 'i', long = "interactive", default_value_t = false)]
    pub interactive: bool,

    /// Skip confirmation prompts and delete all matched targets immediately
    #[arg(short = 'y', long = "yes", default_value_t = false)]
    pub yes: bool,

    /// Only target directories not modified within duration (e.g., `30d`, `2w`, `6m`, `12h`)
    #[arg(short = 'o', long = "older-than", value_name = "DURATION")]
    pub older_than: Option<String>,

    /// Move directories to the OS Trash / Recycle Bin instead of permanent deletion
    #[arg(long = "trash", default_value_t = false)]
    pub trash: bool,

    /// Number of worker threads for scanning and deletion (defaults to logical CPU cores)
    #[arg(short = 'j', long = "threads", value_name = "N")]
    pub threads: Option<usize>,

    /// Exclude paths matching glob patterns (e.g., `--exclude "**/vendor/**"`)
    #[arg(long = "exclude", value_name = "GLOB")]
    pub exclude: Vec<String>,

    /// Emit structured JSON output for scripting and CI/CD pipelines
    #[arg(long = "json", default_value_t = false)]
    pub json: bool,
}

impl CliArgs {
    pub fn scan_node(&self) -> bool {
        self.targets.iter().any(|t| t.includes_node())
    }

    pub fn scan_nuget(&self) -> bool {
        self.targets.iter().any(|t| t.includes_nuget())
    }

    pub fn parsed_older_than(&self) -> Result<Option<Duration>, CleanerError> {
        match &self.older_than {
            None => Ok(None),
            Some(raw) => parse_human_duration(raw).map(Some),
        }
    }
}

/// Parses human-friendly duration strings like `30d`, `2w`, `6m`, `24h`, or standard `humantime` strings.
pub fn parse_human_duration(input: &str) -> Result<Duration, CleanerError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(CleanerError::InvalidDuration(
            input.to_string(),
            "duration cannot be empty".to_string(),
        ));
    }

    // Support shorthand `<number>w` (weeks) and `<number>m` (months = 30d) if single-unit suffix
    if let Some(num_str) = trimmed.strip_suffix('w') {
        if let Ok(weeks) = num_str.trim().parse::<u64>() {
            return Ok(Duration::from_secs(weeks * 7 * 86_400));
        }
    }
    if let Some(num_str) = trimmed.strip_suffix("mo") {
        if let Ok(months) = num_str.trim().parse::<u64>() {
            return Ok(Duration::from_secs(months * 30 * 86_400));
        }
    }

    humantime::parse_duration(trimmed)
        .map_err(|e| CleanerError::InvalidDuration(input.to_string(), e.to_string()))
}
