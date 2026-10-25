use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use indicatif::{ParallelProgressIterator, ProgressBar, ProgressStyle};
use rayon::prelude::*;

use crate::scanner::CleanTarget;

#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct CleanupReport {
    pub deleted_targets: Vec<CleanTarget>,
    pub failed_targets: Vec<(PathBuf, String)>,
    pub total_bytes_freed: u64,
}

/// Deletes the selected `CleanTarget` directories in parallel using either permanent removal
/// (`std::fs::remove_dir_all`) or the OS Trash (`trash::delete`).
pub fn clean_targets(
    targets: &[CleanTarget],
    use_trash: bool,
    show_progress: bool,
) -> CleanupReport {
    if targets.is_empty() {
        return CleanupReport::default();
    }

    let progress_bar = if show_progress {
        let pb = ProgressBar::new(targets.len() as u64);
        pb.set_style(
            ProgressStyle::with_template(
                "🗑️  Deleting {len} directories... [{bar:32.cyan/blue}] {pos}/{len} ({elapsed})",
            )
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("█▓░"),
        );
        pb.enable_steady_tick(Duration::from_millis(80));
        pb
    } else {
        ProgressBar::hidden()
    };

    let outcomes: Vec<Result<CleanTarget, (PathBuf, String)>> = targets
        .par_iter()
        .progress_with(progress_bar.clone())
        .map(|target| {
            let res = if use_trash {
                trash::delete(&target.path).map_err(|e| e.to_string())
            } else {
                fs::remove_dir_all(&target.path).map_err(|e| e.to_string())
            };

            match res {
                Ok(()) => Ok(target.clone()),
                Err(err_msg) => Err((target.path.clone(), err_msg)),
            }
        })
        .collect();

    progress_bar.finish_and_clear();

    let mut report = CleanupReport::default();
    for outcome in outcomes {
        match outcome {
            Ok(deleted) => {
                report.total_bytes_freed =
                    report.total_bytes_freed.saturating_add(deleted.size_bytes);
                report.deleted_targets.push(deleted);
            }
            Err((path, err)) => {
                report.failed_targets.push((path, err));
            }
        }
    }

    report
}
