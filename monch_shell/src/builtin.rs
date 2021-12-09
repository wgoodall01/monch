use crate::cbor_display::format_cbor;
use crate::{exe, types::Ty, Error, Exit, Interpreter, Streams};
use lazy_static::lazy_static;
use monch_io;
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::str::FromStr;
use std::thread;

type StaticBuiltin = &'static (dyn exe::Execute + Sync);

/// Macro to concisely create static references to builtins by heap-allocating them and leaking the
/// references.
///
/// We do this because we only ever initialize the builtins list at most once per process.
macro_rules! static_builtin {
    ($name:expr, $exe:expr) => {{
        let name: &str = $name; // just for the type-check

        // Move the builtin onto the heap, and leak the reference (giving us a `&'static`)
        let sb: StaticBuiltin = Box::leak(Box::new($exe));

        (name, sb)
    }};
}

lazy_static! {
    pub static ref BUILTINS: BTreeMap<&'static str, StaticBuiltin> =
        BTreeMap::from([static_builtin!("cd", Cd), static_builtin!("to", To)]);
}

/// Convenience macro to return an immediate exit code from the [`Execute`] impl of a builtin.
macro_rules! exit {
    ($code:literal) => {
        exit!($crate::exe::Exit::Code(literal))
    };

    ($exit:expr) => {
        return {
            let improc = $crate::exe::ImmediateProc($exit);
            Ok(Box::new(improc))
        }
    };
}

pub struct Cd;

impl exe::Execute for Cd {
    fn execute(
        &self,
        int: &mut Interpreter,
        mut ios: Streams,
        args: &exe::Args,
    ) -> Result<Box<dyn exe::Wait>, Error> {
        let dir = if let [ref dir] = args[..] {
            dir
        } else {
            writeln!(ios.stderr, "monch: cd: too many arguments")?;
            exit!(Exit::FAILURE)
        };

        let workdir = int.current_dir();
        let new_workdir = workdir.join(dir);

        if !new_workdir.is_dir() {
            writeln!(ios.stderr, "monch: cd: {}: no such file or directory", dir)?;
            exit!(Exit::FAILURE)
        }

        let result = int.set_current_dir(new_workdir);

        match result {
            Ok(_) => exit!(Exit::SUCCESS),
            Err(e) => exit!(e.as_exit()),
        }
    }

    fn input_type(&self, _: &exe::Args) -> Ty {
        Ty::Nothing
    }

    fn output_type(&self, _: &exe::Args) -> Ty {
        Ty::Nothing
    }
}

pub struct To;

impl To {
    /// Parse the arguments of a `to` invocation, returning the type we're converting to, or an
    /// error message.
    fn parse_args(args: &exe::Args) -> Result<Ty, Box<dyn std::error::Error>> {
        let type_name = match args[..] {
            [ref type_name] => type_name,
            _ => Err("to: expected one argument only")?,
        };

        Ty::from_str(type_name)
            .map_err(|_| format!("to: '{}' is not a valid type name", type_name).into())
    }
}

impl exe::Execute for To {
    fn execute(
        &self,
        _int: &mut Interpreter,
        mut ios: Streams,
        args: &exe::Args,
    ) -> Result<Box<dyn exe::Wait>, Error> {
        let target_ty = match To::parse_args(args) {
            Ok(ty) => ty,
            Err(err) => {
                let _ = writeln!(ios.stderr, "{}", err);
                exit!(Exit::FAILURE)
            }
        };

        let worker = thread::spawn(move || {
            match target_ty {
                // Format CBOR as text
                Ty::Tty => {
                    let parser = monch_io::InputParser::new(ios.stdin);

                    // Loop over input data
                    for item in parser {
                        let data = match item {
                            Err(e) => {
                                let _ = writeln!(ios.stderr, "to: tty: {}", e);
                                return Exit::FAILURE;
                            }
                            Ok(d) => d,
                        };

                        // Output the item
                        if let Err(e) = format_cbor(&mut ios.stdout, &data) {
                            let _ = writeln!(ios.stderr, "to: {}", e);
                            return Exit::FAILURE;
                        }

                        let _ = writeln!(ios.stderr);
                    }

                    Exit::SUCCESS
                }

                // Pass through CBOR unchanged
                Ty::Cbor => io::copy(&mut ios.stdin, &mut ios.stdout)
                    .map(|_| Exit::SUCCESS)
                    .unwrap_or(Exit::FAILURE),

                // For everything else, bail.
                ty => {
                    let _ = writeln!(ios.stderr, "to: cannot convert to {}", ty);
                    Exit::FAILURE
                }
            }
        });

        Ok(Box::new(worker))
    }

    fn input_type(&self, _: &exe::Args) -> Ty {
        Ty::Cbor
    }

    fn output_type(&self, args: &exe::Args) -> Ty {
        To::parse_args(args).unwrap_or(Ty::Nothing)
    }
}
