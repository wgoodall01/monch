use anyhow::{Context, Error};
use clap::Parser;
use monch_io::{input_stream, put, DataPath, Value, log};

// Note: balls

#[derive(Debug, Parser)]
struct Args {
    /// The pattern to replace
    pattern: String,

    // The replacement for the 
    replacement: String,

    /// Pass in a field for DataPath(s).
    #[clap(short('f'), long, default_value(""))]
    field: DataPath,
    
}

fn main() -> Result<(), Error> {
    let args = Args::parse();

    for string_result in input_stream::<Value>() {
        // Bail if we have an error 
        let val = string_result.context("failed to read string from stdin")?;

        let inner_val = args.field.get_from(val.clone());
        let maybe_string = inner_val.as_text();

        if let Some(string) = maybe_string {

            

            put!(&string.replace(&args.pattern, &args.replacement));

        } else {

            log!("sed: unexpected non-string data item passed in");
            continue;

        }

    }

    Ok(())
}
