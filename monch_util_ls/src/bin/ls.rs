use anyhow::{Context, Error};
use clap::Parser;
use monch_io::{log, put};
use serde::Serialize;
use std::{env, fs, path::PathBuf};

#[derive(Debug, Parser)]
struct Args {
    directory: Option<PathBuf>,

    /// Do not ignore directory entries starting with `.`
    #[clap(short('a'), long)]
    all: bool,

    /// Include file metadata
    #[clap(short('l'), long)]
    long: bool,
}

#[derive(Serialize)]
struct DirEntry {
    name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
}

fn main() -> Result<(), Error> {
    let args = Args::parse();

    // Determine the directory we're reading from
    let dir = match args.directory {
        Some(dir) => dir,
        None => env::current_dir().context("no working directory")?,
    };

    // Read the directory
    let entries = fs::read_dir(dir).context("failed to read directory")?;

    // Iterate over the directory entries
    for entry_result in entries {
        let entry = entry_result.context("failed to read directory entry")?;

        // Get the filename, and check that it's valid Unicode.
        let filename_os = entry.file_name();
        let name: String = match filename_os.into_string() {
            Ok(s) => s,

            // If the filename wasn't valid Unicode, log a warning, and skip to the next file.
            Err(original) => {
                log!(
                    "ls: encountered file with non-utf8 name: {}",
                    original.to_string_lossy()
                );
                continue; // skip to the next file
            }
        };

        // Depending on the `-a` flag, skip hidden entries (starting with a dot)
        if !args.all && name.starts_with('.') {
            continue; // skip this file
        }

        // Depending on if `-l` was passed, write extended information or just the filename to
        // stdout.
        if args.long {
            // Write extended information about the file, in an object like `{name, kind, ..}`.

            let meta = entry.metadata().context("failed to read file metadata")?;

            // Based on file metadata, come up with a type
            let kind = if meta.is_dir() {
                "Dir"
            } else if meta.is_file() {
                "File"
            } else {
                "Unknown"
            };

            // Output the filename and extended information.
            put!({
                "name" => name,
                "kind" => kind,
            });
        } else {
            // Output the filename and nothing else.
            put!(&name);
        }
    }

    Ok(())
}
