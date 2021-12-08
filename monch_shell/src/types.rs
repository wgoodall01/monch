use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ty {
    /// Anything. Can receive data from any source.
    Any,

    /// Binary data we don't know the type of.
    Unknown,

    /// No data output, or no data read from stdin.
    Nothing,

    /// Stream of CBOR-encoded binary data
    Cbor,
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Ty::*;
        let name = match self {
            Any => "[any]",
            Unknown => "[unknown]",
            Nothing => "[nothing]",
            Cbor => "cbor",
        };
        write!(f, "{}", name)
    }
}

/// If we can connect output of type [`from`] to an input stream of type [`to`]
pub fn can_connect(from: Ty, to: Ty) -> bool {
    use Ty::*;
    match (from, to) {
        // If we're receiving [`Any`] data, we can always connect.
        (_, Any) => true,

        // We can always connect data to itself.
        (x, y) if x == y => true,

        // It's fine to pipe something into nothing.
        (_, Nothing) => true,

        // Otherwise, we can't connect.
        _ => false,
    }
}
