use anyhow::Result;
use clap::{Parser, Subcommand};
use diffpatch::{Differ, MultifilePatch, MultifilePatcher, Patch, Patcher};

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
            // Change to the specified directory if provided
            let original_dir = if let Some(dir) = directory {
                let current_dir = std::env::current_dir()?;
                std::env::set_current_dir(&dir)?;
                Some(current_dir)
            } else {
                None
            };

            // Parse and apply the multifile patch
            let multifile_patch = MultifilePatch::parse_from_file(patch_path)?;
            let patcher = MultifilePatcher::new(multifile_patch);
            let written_files = patcher.apply_and_write(reverse)?;

            println!("Successfully updated {} files:", written_files.len());
            for file in written_files {
                println!("  {}", file);
            }

            // Change back to the original directory if we changed it
            if let Some(dir) = original_dir {
                std::env::set_current_dir(dir)?;
            }
        }
    }

    Ok(())
}
