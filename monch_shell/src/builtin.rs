use crate::{exe, types::Ty, Error, Exit, Interpreter, Streams};
use lazy_static::lazy_static;
use std::collections::BTreeMap;
use std::io::Write;

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
        BTreeMap::from([static_builtin!("cd", Cd),]);
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
        args: &[&str],
    ) -> Result<Box<dyn exe::Wait>, Error> {
        let dir = if let [dir] = *args {
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

    fn input_type(&self, _: &[&str]) -> Ty {
        Ty::Nothing
    }

    fn output_type(&self, _: &[&str]) -> Ty {
        Ty::Nothing
    }
}
