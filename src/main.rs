use anyhow::Result;
use clap::{Parser, Subcommand};
use diffpatch::{ApplyResult, Differ, MultifilePatch, MultifilePatcher, Patch, Patcher};

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
        #[arg(short = 'i', long)]
        old: PathBuf,

        /// The new file
        #[arg(short, long)]
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
        #[arg(short, long)]
        patch: PathBuf,

        /// The file to apply the patch to
        #[arg(short, long)]
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
        #[arg(short, long)]
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
        } => {
            let old_content = fs::read_to_string(&old)?;
            let new_content = fs::read_to_string(&new)?;

            let differ = Differ::new(&old_content, &new_content).context_lines(context);
            let mut patch = differ.generate();

            // Set filenames based on the paths
            patch.old_file = old
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("original")
                .to_string();

            patch.new_file = new
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("modified")
                .to_string();

            let result = patch.to_string();

            match output {
                Some(path) => fs::write(path, result)?,
                None => println!("{}", result),
            }
        }

        Commands::Apply {
            patch: patch_path,
            file,
            output,
            reverse,
        } => {
            let patch_content = fs::read_to_string(&patch_path)?;
            let file_content = fs::read_to_string(&file)?;

            // Parse the patch
            let patch = Patch::parse(&patch_content)?;

            // Apply the patch
            let patcher = Patcher::new(patch);
            let result = patcher.apply(&file_content, reverse)?;

            match output {
                Some(path) => fs::write(path, result)?,
                None => println!("{}", result),
            }
        }

        Commands::ApplyMulti {
            patch: patch_path,
            directory,
            reverse,
        } => {
            // Determine the root directory for applying patches.
            let root_dir = directory.unwrap_or(std::env::current_dir()?);

            // Parse the multifile patch.
            let multifile_patch = MultifilePatch::parse_from_file(&patch_path)?;

            // Create the patcher with the specified root directory.
            let patcher = MultifilePatcher::with_root(multifile_patch, &root_dir);

            // Apply the patches and write changes.
            let results = patcher.apply_and_write(reverse)?;

            // Report the results to the user.
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
                        eprintln!("  Failed: {} - {}", path, error); // Use eprintln for errors
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
                // Indicate overall failure if any patch failed.
                // Consider returning a specific error code or using anyhow::bail!
                // std::process::exit(1);
                anyhow::bail!("{} patches failed to apply.", failed_count);
            }
        }
    }

    Ok(())
}
