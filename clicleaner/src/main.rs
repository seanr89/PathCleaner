use std::io::IsTerminal;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use clap::Parser;
use console::style;
use indicatif::{ProgressBar, ProgressStyle};

use clicleaner::cleaner::{clean_targets, CleanupReport};
use clicleaner::cli::CliArgs;
use clicleaner::scanner::{scan_directory, CleanTarget, ScanConfig};
use clicleaner::ui::{print_cleanup_report, print_targets_table, prompt_interactive_selection};

#[derive(serde::Serialize)]
struct JsonOutput<'a> {
    root_path: &'a std::path::Path,
    dry_run: bool,
    discovered_targets: &'a [CleanTarget],
    total_reclaimable_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    cleanup_report: Option<CleanupReport>,
}

fn main() -> ExitCode {
    let args = CliArgs::parse();

    if let Err(err) = run(args) {
        eprintln!("{} {err}", style("error:").red().bold());
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

fn run(args: CliArgs) -> anyhow::Result<()> {
    if let Some(threads) = args.threads {
        if threads > 0 {
            let _ = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build_global();
        }
    }

    let older_than = args.parsed_older_than()?;
    let scan_config = ScanConfig {
        root_path: args.target_path.clone(),
        scan_node: args.scan_node(),
        scan_nuget: args.scan_nuget(),
        include_obj: args.include_obj,
        strict_node: args.strict,
        older_than,
        exclude_patterns: args.exclude.clone(),
    };

    let is_tty = std::io::stdout().is_terminal() && std::io::stdin().is_terminal();

    let spinner = if !args.json && is_tty {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::with_template("{spinner:.cyan} 🔍 Scanning {msg} ...")
                .unwrap_or_else(|_| ProgressStyle::default_spinner()),
        );
        pb.set_message(args.target_path.display().to_string());
        pb.enable_steady_tick(Duration::from_millis(80));
        Some(pb)
    } else {
        if !args.json {
            println!("🔍 Scanning {} ...", args.target_path.display());
        }
        None
    };

    let start = Instant::now();
    let targets = scan_directory(&scan_config)?;
    let elapsed = start.elapsed();

    if let Some(pb) = spinner {
        pb.finish_and_clear();
    }

    let total_reclaimable_bytes: u64 = targets.iter().map(|t| t.size_bytes).sum();

    if args.json {
        let report = if !args.dry_run && args.yes && !targets.is_empty() {
            Some(clean_targets(&targets, args.trash, false))
        } else {
            None
        };

        let payload = JsonOutput {
            root_path: &args.target_path,
            dry_run: args.dry_run || !args.yes,
            discovered_targets: &targets,
            total_reclaimable_bytes,
            cleanup_report: report,
        };
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }

    if targets.is_empty() {
        println!(
            "{} No cleanable `node_modules` or .NET/NuGet directories found in {} ({:.0?}).",
            style("✔").green().bold(),
            style(args.target_path.display()).cyan(),
            elapsed
        );
        return Ok(());
    }

    println!(
        "{} Found {} cleanable director{} in {:.0?}\n",
        style("✔").green().bold(),
        style(targets.len()).bold(),
        if targets.len() == 1 { "y" } else { "ies" },
        elapsed
    );

    print_targets_table(&args.target_path, &targets);

    if args.dry_run {
        println!(
            "\n{}",
            style("Dry-run mode enabled: no directories were deleted.").dim()
        );
        return Ok(());
    }

    let selected_targets = if args.yes {
        targets
    } else if args.interactive || is_tty {
        println!();
        prompt_interactive_selection(&args.target_path, &targets)?
    } else {
        println!(
            "\n{}",
            style("Non-interactive session detected. Pass `--yes` (`-y`) to delete automatically, or `--dry-run` (`-d`) to preview only.").yellow()
        );
        return Ok(());
    };

    if selected_targets.is_empty() {
        println!(
            "\n{}",
            style("Aborted or no directories selected. Nothing was deleted.").yellow()
        );
        return Ok(());
    }

    let report = clean_targets(&selected_targets, args.trash, is_tty);
    print_cleanup_report(&report, args.trash);

    if !report.failed_targets.is_empty() {
        anyhow::bail!(
            "{} directory removal(s) failed",
            report.failed_targets.len()
        );
    }

    Ok(())
}
