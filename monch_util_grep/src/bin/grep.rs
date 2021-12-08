use anyhow::{Context, Error};
use clap::Parser;
use monch_io::{input_stream, put, DataPath, Value, log};

// Note: balls

#[derive(Debug, Parser)]
struct Args {
    /// The path to the field to extract, like '.outerMap.innerMap.2'
    pattern: String,

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

            if string.contains(&args.pattern) {
                put!(&val);
            }

        } else {

            log!("grep: unexpected non-string data item passed in");
            continue;

        }

    }

    Ok(())
}
