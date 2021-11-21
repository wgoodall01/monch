use crate::{interpreter::Interpreter, streams::Streams, Error};
use std::{fmt, process, thread};
use which::which;

/// An executable program or builtin.
pub trait Execute {
    /// Execute the program, given a mutable reference to the parent Interpreter.
    ///
    /// Because each [`Execute`] gets an exclusive reference to its Interpreter, they only can
    /// be launched one at a time. That's fine--heavy work will be done asynchronously.
    fn execute(
        &self,
        int: &mut Interpreter,
        ios: Streams,
        args: &[&str],
    ) -> Result<Box<dyn Wait>, Error>;
}

/// An implementation of [`Execute`] that will search for an external binary and execute it as a
/// child process.
pub struct ExternalExecutable(pub String);

impl Execute for ExternalExecutable {
    fn execute(
        &self,
        int: &mut Interpreter,
        ios: Streams,
        args: &[&str],
    ) -> Result<Box<dyn Wait>, Error> {
        let command_name = self.0.as_str();

        // Resolve the name of that binary path
        let bin_path = which(&command_name).map_err(|source| Error::ResolveBinary {
            cmd: command_name.to_string(),
            source,
        })?;

        // Create the command
        let mut cmd = process::Command::new(bin_path);
        cmd.args(args);

        // Set the working directory to that of the interpreter
        cmd.current_dir(int.current_dir());

        // Hook up the IO
        cmd.stdin(ios.stdin);
        cmd.stdout(ios.stdout);
        cmd.stderr(ios.stderr);

        // Start the child, and return its join handle.
        let wait_handle = Box::new(cmd.spawn()?);
        Ok(wait_handle)
    }
}

/// An in-flight process, either an external process, or a thread in the interpreter.
pub trait Wait {
    /// Block until the process has completed, returning its exit code, or an internal error.
    fn wait(self: Box<Self>) -> Result<Exit, Error>;
}

/// A simple [`Wait`] implementation, which immediately returns an exit code.
pub struct ImmediateProc(pub Exit);

impl Wait for ImmediateProc {
    /// Immediately return the exit code.
    fn wait(self: Box<Self>) -> Result<Exit, Error> {
        Ok(self.0)
    }
}

/// Implement Wait for a [`std::thread::JoinHandle`] returning anything we can convert to an Exit.
impl<E> Wait for thread::JoinHandle<E>
where
    Exit: From<E>,
{
    fn wait(self: Box<Self>) -> Result<Exit, Error> {
        let thread_result = self
            .join()
            .expect("Panic while waiting for an internal thread");
        let exit = Exit::from(thread_result);
        Ok(exit)
    }
}

/// Wait for a child process to exit.
impl Wait for process::Child {
    fn wait(mut self: Box<Self>) -> Result<Exit, Error> {
        let status = process::Child::wait(&mut self).map_err(Error::ExecutionFailed)?;
        Ok(Exit::from(status))
    }
}

/// Represents a process's exit status.
///
/// We optionally store the exit code, because if the process was terminated by a signal, it
/// doesn't actually return an exit code. We don't extract any information about the signal here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit {
    /// The processes exited with a code. This happens when it calls `exit()`, or when it returns
    /// from `main()`. On Windows, all processes exit with a code---but on POSIX, processes that
    /// have been killed due to an unhandled signal do not exit with a code. Instead, they will
    /// return the `Signal(signo)` variant.
    Code(u32),

    /// The process was killed due to an unhandled signal. We store the signal number here.
    Signal(u32),
}

impl Exit {
    /// The exit result of a successful process.
    pub const SUCCESS: Exit = Exit::Code(0);

    /// The main exit result of a failed process.
    pub const FAILURE: Exit = Exit::Code(1);

    /// The exit status for "misuse of a builtin" or bad syntax.
    ///
    /// See the [bash docs](https://tldp.org/LDP/abs/html/exitcodes.html) for details.
    pub const BAD_SYNTAX: Exit = Exit::Code(2);

    /// The exit given when a command was found, but could not be executed.
    pub const COULD_NOT_EXECUTE: Exit = Exit::Code(126);

    /// The exit given when a command binary could not be found.
    pub const COMMAND_NOT_FOUND: Exit = Exit::Code(127);

    /// Get the exit code from the process, if there is one.
    pub fn code(&self) -> Option<u32> {
        match self {
            Exit::Code(c) => Some(*c),
            Exit::Signal(_) => None,
        }
    }

    /// Get the signal number that killed the processs, if the process was killed due to an
    /// unhandled signal.
    pub fn signal(&self) -> Option<u32> {
        match self {
            Exit::Signal(signo) => Some(*signo),
            Exit::Code(_) => None,
        }
    }

    /// Returns whether this signal represents a successful exit: a zero status code.
    pub fn success(&self) -> bool {
        *self == Self::SUCCESS
    }

    /// Returns the "worst" [`Exit`] of the two, similar to Bash's short-circuiting `&&` operator.
    pub fn reduce_worst(a: Exit, b: Exit) -> Exit {
        if !a.success() {
            a
        } else {
            b
        }
    }
}

impl fmt::Display for Exit {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Exit::Code(c) => write!(fmt, "{}", c),
            Exit::Signal(s) => write!(fmt, "signal({})", s),
        }
    }
}

impl From<process::ExitStatus> for Exit {
    fn from(status: process::ExitStatus) -> Exit {
        #![allow(clippy::needless_return)]

        // If we have an exit code, use it.
        if let Some(code) = status.code() {
            // Binary-cast the signed status code to a u32 here.
            return Exit::Code(code as u32);
        }

        #[cfg(target_family = "windows")]
        {
            unreachable!("Windows process exited without an exit code. This is impossible. ")
        }

        #[cfg(target_family = "unix")]
        {
            use std::os::unix::process::ExitStatusExt;
            let signal_number = status.signal()
                .unwrap_or_else(|| unreachable!("POSIX process exited with neither an exit code nor a signal. This is impossible."));

            return Exit::Signal(signal_number as u32);
        }

        #[cfg(not(any(target_family = "windows", target_family = "unix")))]
        compile_error!("cannot interpret exit codes on this platform")
    }
}
