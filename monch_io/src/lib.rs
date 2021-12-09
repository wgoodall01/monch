use serde::{Deserialize, Serialize};
use std::{io, iter};
use thiserror::Error;

/// Re-export the `cbor!` macro to implement our `put!` macro
pub use ciborium;
pub use ciborium::cbor;
pub use ciborium::value::Value;

mod path;
pub use path::DataPath;

#[derive(Error, Debug)]
pub enum Error {
    #[error("{0}")]
    Io(#[from] io::Error),

    #[error("constructing output value: {0}")]
    ConstructValue(#[from] ciborium::value::Error),

    #[error("writing object: {0}")]
    Serialize(#[from] ciborium::ser::Error<io::Error>),

    #[error("reading object: {0}")]
    Deserialize(#[from] ciborium::de::Error<io::Error>),
}

/// Logs a message for humans to standard error.
#[macro_export]
macro_rules! log {
    ($($arg:tt) *) => { eprintln!($($arg) *) }
}

/// Writes an object for machines to standard out.
#[macro_export]
macro_rules! try_put {
    // Take invocations like `put!({"key" => "value"})` or `put!(["a", "list""])`
    ({ $($toks:tt) * }) => { ::monch_io::try_put!(@ cbor { $($toks) * }) };
    ([ $($toks:tt) * ]) => { ::monch_io::try_put!(@ cbor [ $($toks) * ]) };

    (@ cbor $($toks:tt) *) => {{
        use monch_io::{cbor, Error, try_put};

        let result: Result<(), Error> = cbor!($($toks) *)
            .map_err(|e| Error::ConstructValue(e))
            .and_then(|val| try_put!(&val));

        result
    }};

    ($value:expr) => { ::monch_io::write($value) };
}

/// Writes an object for machines to standard out. If the write fails, panic.
#[macro_export]
macro_rules! put {
    ( $($toks:tt) * ) => {{
        let result = ::monch_io::try_put!( $($toks) * );

        if let Err(::monch_io::Error::Serialize(::monch_io::ciborium::ser::Error::Io(ref ioe))) = result {
            if ioe.kind() == ::std::io::ErrorKind::BrokenPipe {
                ::std::process::exit(0); // gracefully die on a SIGPIPE
            }
        }

        if let Err(e) = result {
            eprintln!("failed to write object: {}", e);
            ::std::process::exit(1);
        }

    }};
}

/// Write a serializable object to structured stdout.
pub fn write<T: Serialize>(object: &T) -> Result<(), Error> {
    ciborium::ser::into_writer(object, io::stdout())?;
    Ok(())
}

/// Read a deserializable object from structured stdin.
pub fn read_one<'a, T: Deserialize<'a>>() -> Result<T, Error> {
    let obj = ciborium::de::from_reader(io::stdin())?;
    Ok(obj)
}

/// Read a series of deserializable objects from structured stdin, stopping when stdin is closed.
pub fn input_stream<T: Deserialize<'static>>() -> impl Iterator<Item = Result<T, Error>> {
    InputParser::new(io::stdin())
}

pub struct InputParser<T, R> {
    buffer: io::BufReader<R>,

    // so that we can use the T generic without storing a T
    _phantom_type: std::marker::PhantomData<T>,
}

impl<T, R: io::Read> InputParser<T, R> {
    pub fn new(reader: R) -> Self {
        // 64-byte input buffer. Short because input lines are short.
        let buffer = io::BufReader::with_capacity(64, reader);

        // Construct an input parser iterator
        InputParser {
            buffer,
            _phantom_type: Default::default(),
        }
    }
}

impl<T: Deserialize<'static>, R: io::Read> iter::Iterator for InputParser<T, R> {
    type Item = Result<T, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        use std::io::BufRead;

        // Try to read the next 64 bytes of data into the buffer.
        // This also lets us check for valid EOFs.
        let readahead = self.buffer.fill_buf();
        match readahead {
            // Pass through IO errors to the calling program.
            Err(e) => return Some(Err(Error::Io(e))),

            // Handle EOFs by stopping iteration
            Ok(buf) if buf.is_empty() => return None,

            // Everything went well, try to read the object.
            Ok(_) => {}
        }

        // Attempt to read one object.
        let read_result = ciborium::de::from_reader(&mut self.buffer);
        match read_result {
            // The object read successfully.
            Ok(obj) => Some(Ok(obj)),

            // There was an error parsing, pass it through to userspace.
            Err(e) => Some(Err(Error::Deserialize(e))),
        }
    }
}
