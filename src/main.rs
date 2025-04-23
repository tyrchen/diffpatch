use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use diffpatch::{patcher::Patcher, ApplyResult, Differ, MultifilePatch, MultifilePatcher, Patch};
use diffpatch::{DiffAlgorithm, PatchAlgorithm};
use std::fs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about = "A tool for generating and applying patches")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a patch from two files
    Generate {
        /// The original file
        old: PathBuf,

        /// The new file
        new: PathBuf,

        /// The output patch file (defaults to stdout if not provided)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Number of context lines to include
        #[arg(short, long, default_value_t = 3)]
        context: usize,
    },

    /// Apply a patch to a file
    Apply {
        /// The patch file to apply
        patch: PathBuf,

        /// The file to apply the patch to
        file: PathBuf,

        /// The output file (defaults to stdout if not provided)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Reverse the patch
        #[arg(short, long, default_value_t = false)]
        reverse: bool,
    },

    /// Apply a multi-file patch
    ApplyMulti {
        /// The patch file to apply
        patch: PathBuf,

        /// The directory to apply patches in (defaults to current directory)
        #[arg(short, long)]
        directory: Option<PathBuf>,

        /// Reverse the patch
        #[arg(short, long, default_value_t = false)]
        reverse: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Generate {
            old,
            new,
            output,
            context,
        } => handle_generate(old, new, output, context),
        Commands::Apply {
            patch,
            file,
            output,
            reverse,
        } => handle_apply(patch, file, output, reverse),
        Commands::ApplyMulti {
            patch,
            directory,
            reverse,
        } => handle_apply_multi(patch, directory, reverse),
    }
}

// Helper function to write output to file or stdout
fn write_output(output_path: Option<PathBuf>, content: &str) -> Result<()> {
    match output_path {
        Some(path) => fs::write(&path, content)
            .with_context(|| format!("Failed to write output to file: {:?}", path)),
        None => {
            println!("{}", content);
            Ok(()) // Ensure Ok(()) is returned for the None case
        }
    }
}

fn handle_generate(
    old_path: PathBuf,
    new_path: PathBuf,
    output_path: Option<PathBuf>,
    context: usize,
) -> Result<()> {
    let old_content = fs::read_to_string(&old_path)
        .with_context(|| format!("Failed to read old file: {:?}", old_path))?;
    let new_content = fs::read_to_string(&new_path)
        .with_context(|| format!("Failed to read new file: {:?}", new_path))?;

    let differ = Differ::new(&old_content, &new_content).context_lines(context);
    let patch = differ.generate();

    let result = patch.to_string();
    write_output(output_path, &result)
}

fn handle_apply(
    patch_path: PathBuf,
    file_path: PathBuf,
    output_path: Option<PathBuf>,
    reverse: bool,
) -> Result<()> {
    let patch_content = fs::read_to_string(&patch_path)
        .with_context(|| format!("Failed to read patch file: {:?}", patch_path))?;
    let file_content = fs::read_to_string(&file_path)
        .with_context(|| format!("Failed to read target file: {:?}", file_path))?;

    let patch = Patch::parse(&patch_content)?;
    let patcher = Patcher::new(patch);
    let result = patcher.apply(&file_content, reverse)?;

    write_output(output_path, &result)
}

fn handle_apply_multi(
    patch_path: PathBuf,
    directory: Option<PathBuf>,
    reverse: bool,
) -> Result<()> {
    let root_dir = directory.unwrap_or(std::env::current_dir()?);
    let multifile_patch = MultifilePatch::parse_from_file(&patch_path)
        .with_context(|| format!("Failed to parse multi-file patch: {:?}", patch_path))?;
    let patcher = MultifilePatcher::with_root(multifile_patch, &root_dir);
    let results = patcher.apply_and_write(reverse)?;

    let mut applied_count = 0;
    let mut deleted_count = 0;
    let mut skipped_count = 0;
    let mut failed_count = 0;

    println!("Patch application results:");
    for result in results {
        match result {
            ApplyResult::Applied(file) => {
                println!(
                    "  Applied: {} {}",
                    file.path,
                    if file.is_new { "(new file)" } else { "" }
                );
                applied_count += 1;
            }
            ApplyResult::Deleted(path) => {
                println!("  Deleted: {}", path);
                deleted_count += 1;
            }
            ApplyResult::Skipped(reason) => {
                println!("  Skipped: {}", reason);
                skipped_count += 1;
            }
            ApplyResult::Failed(path, error) => {
                eprintln!("  Failed: {} - {}", path, error);
                failed_count += 1;
            }
        }
    }

    println!("\nSummary:");
    println!("  {} applied/modified", applied_count);
    println!("  {} deleted", deleted_count);
    println!("  {} skipped", skipped_count);
    println!("  {} failed", failed_count);

    if failed_count > 0 {
        anyhow::bail!("{} patches failed to apply.", failed_count);
    }

    Ok(())
}
