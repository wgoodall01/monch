use anyhow::{Context, Error};
use clap::Parser;
use monch_io::{input_stream, put, DataPath, Value};

// Note: the [`DataPath`] in [`Args`] has an implementation of [`FromStr`] that allows the [`Parser`] derive to
// figure out how to parse it from the command line arguments.

#[derive(Debug, Parser)]
struct Args {
    /// The path to the field to extract, like '.outerMap.innerMap.2'
    path: DataPath,
}

fn main() -> Result<(), Error> {
    let args = Args::parse();

    // Read CBOR objects from stdin
    for obj_result in input_stream::<Value>() {
        // Bail if we have an unhandled error.
        let obj = obj_result.context("failed to read object from stdin")?;

        // Get the data
        let selected_data = args.path.get_from(obj);

        // Write the data
        put!(&selected_data);
    }

    Ok(())
}
