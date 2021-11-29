use serde::{Deserialize, Serialize};
use std::io;
use thiserror::Error;

/// Re-export the `cbor!` macro to implement our `put!` macro
pub use ciborium::cbor;

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
    ( $($toks:tt) * ) => { ::monch_io::try_put!( $($toks) * ).expect("failed to put object") };
}

/// Write a serializable object to structured stdout.
pub fn write<T: Serialize>(object: &T) -> Result<(), Error> {
    ciborium::ser::into_writer(object, io::stdout())?;
    Ok(())
}

/// Read a deserializable object from structured stdin.
pub fn read<'a, T: Deserialize<'a>>() -> Result<T, Error> {
    let obj = ciborium::de::from_reader(io::stdin())?;
    Ok(obj)
}
