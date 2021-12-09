use ciborium::value::Value;
use std::fmt;
use std::str::FromStr;

/// Represents a path to get some nested data, like `.outer.inner.12.field`.
#[derive(Debug, Clone)]
pub struct DataPath(Vec<Value>);

impl DataPath {
    /// Parse a `jq`-like string path, in the form `outer.inner.1`.
    ///
    /// Works by converting each `.`-separated path segment into either a CBOR
    /// [`Integer`](ciborium::value::Integer), or a String key.
    pub fn parse(path: &str) -> DataPath {
        let parsed_parts = path
            .split('.')
            .filter(|part| !part.is_empty())
            .map(|part| {
                // For each `.`-separated path segment...

                // Try to parse it as an integer
                if let Ok(int) = u64::from_str(part) {
                    // return CBOR varint encoding
                    return Value::Integer(int.into());
                }

                // If it's not an integer, treat it as a string key.
                Value::Text(part.to_string())
            })
            .collect();

        DataPath(parsed_parts)
    }

    /// If this path is non-empty, split off the first element.
    pub fn split_head(&self) -> Option<(Value, DataPath)> {
        self.0
            .split_first()
            .map(|(head, tail)| (head.clone(), DataPath(tail.into())))
    }

    /// Use this path to get an inner value from a CBOR [`Value`].
    ///
    /// If the value is not found, returns [`Value::Null`] instead.
    pub fn get_from(&self, value: Value) -> Value {
        // Pass through CBOR tags unchanged, regardless of the path
        if let Value::Tag(_tag, val) = value {
            // Get the reference inside the box
            return self.get_from(*val);
        }

        // Pull off the key from the path.
        let (key, rest) = match self.split_head() {
            Some(x) => x,

            // If the path is empty (split_head returns None)
            // then we return the current element. This is the
            // base case.
            None => return value,
        };

        // Otherwise, evaluate the key against the value.
        match value {
            // Index into arrays, recursing.
            Value::Array(mut array) => match key {
                Value::Integer(key) => {
                    let key_usize = usize::try_from(key).expect("exceptionally big key");

                    // Bounds-check the index
                    if key_usize < array.len() {
                        // Get the element and recurse on it.
                        let inner = array.swap_remove(key_usize);
                        rest.get_from(inner) // recurse
                    } else {
                        // Index was out of bounds. Return null.
                        Value::Null
                    }
                }
                _ => Value::Null, // any other key type won't work
            },

            // Key into maps, recursing.
            // (take the first matching element, ciborium uses KV lists instead of map structures)
            Value::Map(map) => {
                let inner = map
                    .into_iter()
                    .find(|(k, _v)| *k == key)
                    .map(|(_k, v)| v)
                    .unwrap_or(Value::Null);

                rest.get_from(inner) // recurse
            }

            // For terminal types, return Null. Because Null indexed at anything also returns Null,
            // this means you can do `.notfound.whatever.whatever` without error (you just get Null).
            // Note: we don't support indexing into strings or byte arrays right now
            Value::Null
            | Value::Integer(_)
            | Value::Float(_)
            | Value::Bool(_)
            | Value::Bytes(_)
            | Value::Text(_) => Value::Null,

            // We handle tags transparently earlier
            Value::Tag(_, _) => unreachable!("unexpected tag value, handled at the top of the fn"),

            // Finally, Value is non-exhaustive---so cover the any case with a Null.
            _ => Value::Null,
        }
    }
}

impl fmt::Display for DataPath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for part in &self.0 {
            match part {
                Value::Text(s) => write!(f, "{}", s)?,
                Value::Integer(i) => write!(f, "{}", u64::try_from(*i).unwrap())?,
                _ => unreachable!("format unknown path part"),
            }
        }
        Ok(())
    }
}

impl From<&str> for DataPath {
    fn from(text: &str) -> DataPath {
        DataPath::parse(text)
    }
}

impl FromStr for DataPath {
    type Err = String; // never constructed

    fn from_str(text: &str) -> Result<DataPath, Self::Err> {
        Ok(DataPath::parse(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ciborium::cbor;

    #[test]
    fn parse_basic_path() {
        let raw = ".outer.inner..200";
        let parsed = DataPath::parse(raw);

        let expected = vec![
            Value::Text("outer".into()),
            Value::Text("inner".into()),
            Value::Integer(200.into()),
        ];

        assert_eq!(parsed.0, expected);
    }

    #[test]
    fn parse_empty_path() {
        assert!(DataPath::parse(".....").0.is_empty());
    }

    #[test]
    fn get_tag() {
        let value = Value::Tag(1234, Box::new(Value::Text("working".into())));

        assert_eq!(
            DataPath(vec![]).get_from(value.clone()),
            Value::Text("working".into())
        );

        assert_eq!(
            DataPath::parse(".some.deep.nest").get_from(value.clone()),
            Value::Null
        );
    }

    #[test]
    fn get_nested() {
        let value = cbor!({
            "outer" => {
                "med" => {
                    "inner" => [
                        "zero",
                        "one",
                        "two"
                    ]
                }
            }
        })
        .unwrap();

        assert_eq!(
            DataPath::parse("outer.med.inner.1")
                .get_from(value.clone())
                .as_text(),
            Some("one")
        );

        assert!(DataPath::parse("..outer.....")
            .get_from(value.clone())
            .is_map());

        assert!(DataPath::parse(".outer.med")
            .get_from(value.clone())
            .is_map());

        assert!(DataPath::parse("outer.med.inner")
            .get_from(value.clone())
            .is_array());

        assert_eq!(
            DataPath::parse("outer.med.inner.1000.10").get_from(value.clone()),
            Value::Null
        );

        assert_eq!(
            DataPath::parse("outer.med.nope.not found.hahahah try again").get_from(value.clone()),
            Value::Null
        );
    }
}
