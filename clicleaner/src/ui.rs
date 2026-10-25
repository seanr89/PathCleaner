use std::path::Path;
use std::time::SystemTime;

use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Cell, CellAlignment, Color, ContentArrangement, Table};
use console::style;
use dialoguer::theme::ColorfulTheme;
use dialoguer::MultiSelect;
use humansize::{format_size, DECIMAL};

use crate::cleaner::CleanupReport;
use crate::scanner::CleanTarget;

/// Formats a byte count into a human-readable string (e.g. `842.10 MB`).
pub fn format_bytes(bytes: u64) -> String {
    format_size(bytes, DECIMAL)
}

/// Formats a `SystemTime` relative to now (e.g., `12 days ago`, `3 hours ago`, `just now`).
pub fn format_relative_time(mtime: Option<SystemTime>) -> String {
    let Some(time) = mtime else {
        return "unknown".to_string();
    };

    let Ok(elapsed) = SystemTime::now().duration_since(time) else {
        return "just now".to_string();
    };

    let secs = elapsed.as_secs();
    if secs < 60 {
        "just now".to_string()
    } else if secs < 3_600 {
        let mins = secs / 60;
        format!("{mins} min{} ago", if mins == 1 { "" } else { "s" })
    } else if secs < 86_400 {
        let hours = secs / 3_600;
        format!("{hours} hour{} ago", if hours == 1 { "" } else { "s" })
    } else {
        let days = secs / 86_400;
        format!("{days} day{} ago", if days == 1 { "" } else { "s" })
    }
}

/// Displays a formatted table of discovered cleanable directories and their sizes.
pub fn print_targets_table(root_path: &Path, targets: &[CleanTarget]) {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("#").fg(Color::Cyan),
            Cell::new("Ecosystem").fg(Color::Cyan),
            Cell::new("Kind").fg(Color::Cyan),
            Cell::new("Files").fg(Color::Cyan),
            Cell::new("Last Modified").fg(Color::Cyan),
            Cell::new("Path").fg(Color::Cyan),
            Cell::new("Size").fg(Color::Cyan),
        ]);

    for (idx, target) in targets.iter().enumerate() {
        let display_path = relative_display_path(root_path, &target.path);
        let ecosystem_color = match target.kind.ecosystem_label() {
            "Node.js" => Color::Green,
            _ => Color::Magenta,
        };

        table.add_row(vec![
            Cell::new(idx + 1).set_alignment(CellAlignment::Right),
            Cell::new(target.kind.ecosystem_label()).fg(ecosystem_color),
            Cell::new(target.kind.short_kind()).fg(Color::Yellow),
            Cell::new(target.file_count).set_alignment(CellAlignment::Right),
            Cell::new(format_relative_time(target.last_modified)),
            Cell::new(display_path),
            Cell::new(format_bytes(target.size_bytes))
                .fg(Color::White)
                .set_alignment(CellAlignment::Right),
        ]);
    }

    println!("{table}");

    let total_bytes: u64 = targets.iter().map(|t| t.size_bytes).sum();
    println!(
        "{} {}",
        style("Total reclaimable space:").bold(),
        style(format_bytes(total_bytes)).green().bold()
    );
}

/// Prompts the user with an interactive checkbox selector to choose which directories to delete.
pub fn prompt_interactive_selection(
    root_path: &Path,
    targets: &[CleanTarget],
) -> anyhow::Result<Vec<CleanTarget>> {
    let items: Vec<String> = targets
        .iter()
        .map(|t| {
            let rel = relative_display_path(root_path, &t.path);
            format!(
                "{rel} ({}, {})",
                t.kind. ecosystem_label(),
                format_bytes(t.size_bytes)
            )
        })
        .collect();

    // Pre-select all discovered items by default so the user can toggle off or press Enter
    let defaults = vec![true; items.len()];

    let selections = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select directories to delete [Space to toggle, Enter to confirm]")
        .items(&items)
        .defaults(&defaults)
        .interact_opt()?;

    let Some(indices) = selections else {
        return Ok(Vec::new());
    };

    Ok(indices.into_iter().map(|i| targets[i].clone()).collect())
}

/// Prints the final cleanup report to the console.
pub fn print_cleanup_report(report: &CleanupReport, use_trash: bool) {
    let action_word = if use_trash {
        "Moved to Trash"
    } else {
        "Deleted"
    };

    if !report.deleted_targets.is_empty() {
        println!(
            "\n✨ {} {} {}! Reclaimed {} of disk space.",
            style("Done!").green().bold(),
            action_word,
            style(format!(
                "{} director{}",
                report.deleted_targets.len(),
                if report.deleted_targets.len() == 1 {
                    "y"
                } else {
                    "ies"
                }
            ))
            .bold(),
            style(format_bytes(report.total_bytes_freed)).green().bold()
        );
    } else {
        println!(
            "\n{}",
            style("No directories were deleted.").yellow().bold()
        );
    }

    if !report.failed_targets.is_empty() {
        eprintln!(
            "\n{} Failed to remove {} director{}:",
            style("⚠").red().bold(),
            report.failed_targets.len(),
            if report.failed_targets.len() == 1 {
                "y"
            } else {
                "ies"
            }
        );
        for (path, err) in &report.failed_targets {
            eprintln!("  - {}: {}", path.display(), style(err).red());
        }
    }
}

pub fn relative_display_path(root_path: &Path, target_path: &Path) -> String {
    if let Ok(stripped) = target_path.strip_prefix(root_path) {
        let s = stripped.display().to_string();
        if s.is_empty() {
            target_path.display().to_string()
        } else {
            format!("./{s}")
        }
    } else {
        target_path.display().to_string()
    }
}
