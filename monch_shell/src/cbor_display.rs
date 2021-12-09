use ciborium::value::Value;
use owo_colors::OwoColorize;
use std::io;

/// Write a human-readable inline description of the CBOR object to the output writer
pub fn format_cbor<'w>(out: &mut impl io::Write, val: &Value) -> io::Result<()> {
    use Value::*;
    match val {
        Float(f) => write!(out, "{:0.3}", f.green()),
        Integer(i) => write!(out, "{}", i128::from(*i).green()),
        Text(s) => write!(out, "{}", s),
        Bool(b) => write!(out, "{}", b.purple()),
        Bytes(_) => write!(out, "{}", "(binary data)".italic()),
        Null => write!(out, "{}", "(null)".italic()),
        Tag(t, inner) => {
            write!(out, "{}", format!("(tag {}) ", t).italic())?;
            format_cbor(out, inner)
        }

        Array(arr) => {
            write!(out, "[")?;
            for (i, item) in arr.iter().enumerate() {
                if i != 0 {
                    write!(out, ", ")?;
                }

                format_cbor(out, item)?;
            }
            write!(out, "]")
        }

        Map(pairs) => {
            write!(out, "{}", "{".dimmed())?;
            for (i, (k, v)) in pairs.iter().enumerate() {
                if i != 0 {
                    write!(out, "{}", ", ".dimmed())?;
                }

                match k {
                    Value::Text(s) => write!(out, "{}", format!("{}: ", s).dimmed())?,
                    _ => {
                        write!(out, ": ")?;
                        format_cbor(out, k)?;
                    }
                }

                format_cbor(out, v)?;
            }
            write!(out, "{}", "}".dimmed())
        }

        _ => write!(out, "[cannot display]"),
    }
}
